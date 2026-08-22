# /// script
# requires-python = "==3.10.*"
# dependencies = [
#     "torch==1.13.1",
#     "torchvision==0.14.1",
#     "fastai==1.0.61",
#     "numpy<2",
#     "Pillow==9.5.0",
#     "opencv-python-headless",
#     "onnx",
#     "onnxruntime",
#     "onnxconverter-common==1.14.0",
#     "setuptools<81",  # the vendored fastai in the DeOldify repo imports pkg_resources, removed in newer setuptools
# ]
#
# [tool.uv]
# # fastai v1 requires pynvx (an NVIDIA monitoring lib) on macOS, which has no arm64 wheels and is
# # never imported on the vision inference path — restrict it to a platform where it won't install.
# override-dependencies = ["pynvx==1.0.0; sys_platform == 'linux'"]
# ///
"""Convert a DeOldify generator checkpoint to ONNX opset 17, in FP32 and FP16.

DeOldify (https://github.com/jantic/DeOldify) is a fastai-v1 DynamicUnetWide GAN generator,
so this script runs in a legacy environment (torch 1.13 + fastai 1.0.61 — the newest pair
that still executes fastai v1) and builds the model through DeOldify's own gen_inference_wide.

The exported graph bakes ImageNet normalization and denormalization in, so the ONNX contract
matches the rest of the project: `input` (1,3,560,560) float32 [0,1] gray RGB -> `output`
(1,3,560,560) float32 [0,1] RGB. 560 = the reference stable colorizer's default
render_factor (35) x 16.

Usage:
    uv run scripts/convert_deoldify.py \
        --name jaipur \
        --checkpoint /Users/vegidio/Desktop/jaipur.pth \
        --repo <path to DeOldify clone> \
        --test-image /Users/vegidio/Desktop/test/bw.jpg \
        --out-dir /Users/vegidio/Desktop \
        --image-out-dir /Users/vegidio/Desktop/test
"""

import argparse
import hashlib
import os
import sys
from pathlib import Path

import cv2
import numpy as np

INPUT_SIZE = 560  # render_factor 35 * render_base 16, the stable colorizer's default

IMAGENET_MEAN = [0.485, 0.456, 0.406]
IMAGENET_STD = [0.229, 0.224, 0.225]


def load_model(repo: Path, checkpoint: Path):
    import torch
    from torch import nn

    sys.path.insert(0, str(repo))

    # The device singleton must be set before deoldify.generators is imported.
    from deoldify import device
    from deoldify.device_id import DeviceId

    device.set(device=DeviceId.CPU)

    from deoldify.generators import gen_inference_wide

    # fastai's Learner.load reads <root>/models/<name>.pth; point that at the checkpoint.
    models_dir = repo / "models"
    models_dir.mkdir(exist_ok=True)
    link = models_dir / "ColorizeStable_gen.pth"
    if link.resolve() != checkpoint.resolve():
        if link.exists() or link.is_symlink():
            link.unlink()
        os.symlink(checkpoint, link)

    learn = gen_inference_wide(root_folder=repo, weights_name="ColorizeStable_gen")
    model = learn.model.cpu().float().eval()
    strip_norms(model)

    class Wrapper(nn.Module):
        """Bakes ImageNet normalize/denormalize into the graph and clamps to [0, 1]."""

        def __init__(self, m):
            super().__init__()
            self.m = m
            self.register_buffer("mean", torch.tensor(IMAGENET_MEAN).view(1, 3, 1, 1))
            self.register_buffer("std", torch.tensor(IMAGENET_STD).view(1, 3, 1, 1))

        def forward(self, x):
            y = self.m((x - self.mean) / self.std)
            return (y * self.std + self.mean).clamp(0.0, 1.0)

    return Wrapper(model).eval()


def strip_norms(module) -> None:
    """Remove spectral/weight-norm reparameterizations, baking each effective weight once.

    Lossless for inference (eval-mode norms compute a deterministic fixed weight), and without it the exported graph
    carries BOTH the original weights and the folded copies plus the runtime norm subgraphs — nearly doubling the file.
    """
    import torch.nn.utils as U

    removed = 0
    for m in module.modules():
        for fn in (U.remove_spectral_norm, U.remove_weight_norm):
            try:
                fn(m)
                removed += 1
            except (ValueError, AttributeError, RuntimeError):
                pass
    print(f"stripped {removed} weight reparameterizations")


def preprocess(img_bgr: np.ndarray) -> np.ndarray:
    """Reference DeOldify preprocessing minus the normalization (baked into the graph):
    stretch to a square, ITU-601 luma grayscale, [0,1] CHW."""
    rgb = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2RGB)
    resized = cv2.resize(rgb, (INPUT_SIZE, INPUT_SIZE), interpolation=cv2.INTER_LINEAR)
    luma = (
        0.299 * resized[:, :, 0] + 0.587 * resized[:, :, 1] + 0.114 * resized[:, :, 2]
    ).round().clip(0, 255).astype(np.uint8)
    gray_rgb = np.stack([luma, luma, luma], axis=-1)
    return (gray_rgb.transpose(2, 0, 1)[np.newaxis] / 255.0).astype(np.float32)


def postprocess(model_rgb: np.ndarray, orig_bgr: np.ndarray) -> np.ndarray:
    """Reference DeOldify postprocessing: resize to the original, YUV chroma transfer."""
    h, w = orig_bgr.shape[:2]
    out = (model_rgb[0].transpose(1, 2, 0) * 255.0).round().clip(0, 255).astype(np.uint8)
    out = cv2.resize(out, (w, h), interpolation=cv2.INTER_LINEAR)

    color_yuv = cv2.cvtColor(out, cv2.COLOR_RGB2YUV)
    orig_yuv = cv2.cvtColor(cv2.cvtColor(orig_bgr, cv2.COLOR_BGR2RGB), cv2.COLOR_RGB2YUV)
    hires = np.copy(orig_yuv)
    hires[:, :, 1:3] = color_yuv[:, :, 1:3]
    final_rgb = cv2.cvtColor(hires, cv2.COLOR_YUV2RGB)
    return cv2.cvtColor(final_rgb, cv2.COLOR_RGB2BGR)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def stats(name: str, a: np.ndarray, b: np.ndarray) -> None:
    diff = np.abs(a.astype(np.float64) - b.astype(np.float64))
    nan = int(np.isnan(b).sum()) + int(np.isinf(b).sum())
    print(f"{name}: max|Δ|={diff.max():.5f} mean|Δ|={diff.mean():.6f} nan/inf={nan}")


def export_onnx(wrapper, path: Path) -> None:
    import onnx
    import torch

    dummy = torch.zeros(1, 3, INPUT_SIZE, INPUT_SIZE)
    try:
        torch.onnx.export(
            wrapper, dummy, str(path),
            opset_version=17,
            input_names=["input"], output_names=["output"],
            do_constant_folding=True, dynamic_axes=None,
        )
    except Exception as e:  # torch 1.13 may lack an op at 17; export 16 and upgrade
        print(f"opset 17 export failed ({e}); exporting at 16 and upgrading")
        torch.onnx.export(
            wrapper, dummy, str(path),
            opset_version=16,
            input_names=["input"], output_names=["output"],
            do_constant_folding=True, dynamic_axes=None,
        )
        from onnx import version_converter

        upgraded = version_converter.convert_version(onnx.load(str(path)), 17)
        onnx.save(upgraded, str(path))

    onnx.checker.check_model(str(path))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True, help="model codename, e.g. jaipur")
    parser.add_argument("--repo", type=Path, required=True, help="path to a DeOldify clone")
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--test-image", type=Path, default=Path("/Users/vegidio/Desktop/test/bw.jpg"))
    parser.add_argument("--out-dir", type=Path, default=Path("/Users/vegidio/Desktop"))
    parser.add_argument("--image-out-dir", type=Path, default=Path("/Users/vegidio/Desktop/test"))
    args = parser.parse_args()

    import torch

    fp32_path = args.out_dir / f"cl_{args.name}_fp32.onnx"
    fp16_path = args.out_dir / f"cl_{args.name}_fp16.onnx"
    args.image_out_dir.mkdir(parents=True, exist_ok=True)

    img_bgr = cv2.imread(str(args.test_image))
    if img_bgr is None:
        raise SystemExit(f"cannot read test image: {args.test_image}")
    input_np = preprocess(img_bgr)

    wrapper = load_model(args.repo, args.checkpoint)

    with torch.no_grad():
        torch_out = wrapper(torch.from_numpy(input_np)).numpy()
    cv2.imwrite(str(args.image_out_dir / f"bw_pytorch_{args.name}.jpg"), postprocess(torch_out, img_bgr))
    print(f"pytorch output range: [{torch_out.min():.3f}, {torch_out.max():.3f}]")

    print("exporting fp32 onnx...")
    export_onnx(wrapper, fp32_path)

    import onnx
    from onnxconverter_common import float16

    print("converting to fp16...")
    model_fp16 = float16.convert_float_to_float16(onnx.load(str(fp32_path)), keep_io_types=True)
    onnx.save(model_fp16, str(fp16_path))

    import onnxruntime as ort

    for path, tag in ((fp32_path, "fp32"), (fp16_path, "fp16")):
        sess = ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])
        ort_out = sess.run(["output"], {"input": input_np})[0]
        stats(f"pytorch vs onnx_{tag}", torch_out, ort_out)
        cv2.imwrite(str(args.image_out_dir / f"bw_onnx_{args.name}_{tag}.jpg"), postprocess(ort_out, img_bgr))

    for path in (fp32_path, fp16_path):
        print(f"{path.name}: {path.stat().st_size / 1e6:.1f} MB  sha256={sha256(path)}")


if __name__ == "__main__":
    main()
