# /// script
# requires-python = ">=3.11,<3.13"
# dependencies = [
#     "torch==2.6.0",
#     "timm>=1.0",
#     "numpy<2",  # onnxconverter-common 1.14 still uses np.fromstring, removed in numpy 2
#     "opencv-python-headless",
#     "onnx==1.17.0",
#     "onnxruntime>=1.20",
#     "onnxconverter-common==1.14.0",
# ]
# ///
"""Convert a DDColor ConvNeXt-L checkpoint to ONNX opset 17, in FP32 and FP16.

One script serves every DDColor-based colorization model in this project: --name picks the
model codename (delhi, mumbai, ...) and derives the cl_<name>_<precision>.onnx output files
and the per-model comparison images.

The DDColor architecture code is taken from a local clone of
https://github.com/p1sangmas/re-DDColor (branch ddcolor-development). Importing
`basicsr.archs.ddcolor_arch` from that repo directly would execute `basicsr/__init__.py`,
which drags in the whole training framework, so this script assembles a minimal shim
package with just the arch files and the registry.

Usage:
    uv run scripts/convert_colorization.py \
        --name mumbai \
        --checkpoint /Users/vegidio/Desktop/mumbai.pth \
        --repo <path to re-DDColor clone> \
        --test-image /Users/vegidio/Desktop/test/bw.jpg \
        --out-dir /Users/vegidio/Desktop \
        --image-out-dir /Users/vegidio/Desktop/test
"""

import argparse
import hashlib
import shutil
import sys
import tempfile
from pathlib import Path

import cv2
import numpy as np

INPUT_SIZE = 512


def build_shim(repo: Path, shim_root: Path) -> None:
    """Assemble a minimal `basicsr` package with only what ddcolor_arch imports."""
    archs = shim_root / "basicsr" / "archs"
    utils = shim_root / "basicsr" / "utils"
    archs.mkdir(parents=True)
    utils.mkdir(parents=True)

    (shim_root / "basicsr" / "__init__.py").write_text("")
    (archs / "__init__.py").write_text("")
    (utils / "__init__.py").write_text("")

    src = repo / "basicsr"
    shutil.copy(src / "utils" / "registry.py", utils / "registry.py")
    shutil.copy(src / "archs" / "ddcolor_arch.py", archs / "ddcolor_arch.py")
    shutil.copytree(src / "archs" / "ddcolor_arch_utils", archs / "ddcolor_arch_utils")
    # The repo ships a typo'd `__int__.py`; make it a proper package.
    (archs / "ddcolor_arch_utils" / "__init__.py").write_text("")


def load_model(repo: Path, checkpoint: Path):
    import torch

    shim_root = Path(tempfile.mkdtemp(prefix="ddcolor_shim_"))
    build_shim(repo, shim_root)
    sys.path.insert(0, str(shim_root))

    from basicsr.archs.ddcolor_arch import DDColor

    # Config from re-DDColor inference/colorization_pipeline.py (model_size='large').
    model = DDColor(
        encoder_name="convnext-l",
        decoder_name="MultiScaleColorDecoder",
        input_size=[INPUT_SIZE, INPUT_SIZE],
        num_output_channels=2,
        last_norm="Spectral",
        do_normalize=False,
        num_queries=100,
        num_scales=3,
        dec_layers=9,
    )

    ckpt = torch.load(checkpoint, map_location="cpu", weights_only=True)
    state = ckpt.get("params", ckpt)
    missing, unexpected = model.load_state_dict(state, strict=False)
    print(f"checkpoint: {len(state)} tensors, missing={list(missing)}, unexpected={list(unexpected)}")
    model.eval()
    strip_norms(model)
    return model


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


def preprocess(img_bgr: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Reference DDColor preprocessing. Returns (orig_l (H,W,1), input tensor (1,3,512,512))."""
    img = (img_bgr / 255.0).astype(np.float32)
    orig_l = cv2.cvtColor(img, cv2.COLOR_BGR2Lab)[:, :, :1]

    img_resized = cv2.resize(img, (INPUT_SIZE, INPUT_SIZE))
    img_l = cv2.cvtColor(img_resized, cv2.COLOR_BGR2Lab)[:, :, :1]
    img_gray_lab = np.concatenate((img_l, np.zeros_like(img_l), np.zeros_like(img_l)), axis=-1)
    img_gray_rgb = cv2.cvtColor(img_gray_lab, cv2.COLOR_Lab2RGB)

    tensor = img_gray_rgb.transpose((2, 0, 1))[np.newaxis].astype(np.float32)
    return orig_l, tensor


def postprocess(orig_l: np.ndarray, output_ab: np.ndarray) -> np.ndarray:
    """Reference DDColor postprocessing (bilinear ab upsample). Returns uint8 BGR."""
    h, w = orig_l.shape[:2]
    ab = output_ab[0].transpose(1, 2, 0)  # (512, 512, 2)
    ab = cv2.resize(ab, (w, h), interpolation=cv2.INTER_LINEAR)
    lab = np.concatenate((orig_l, ab), axis=-1)
    bgr = cv2.cvtColor(lab, cv2.COLOR_Lab2BGR)
    return (bgr * 255.0).round().clip(0, 255).astype(np.uint8)


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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True, help="model codename, e.g. delhi or mumbai")
    parser.add_argument("--repo", type=Path, required=True)
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
    orig_l, input_np = preprocess(img_bgr)

    model = load_model(args.repo, args.checkpoint)

    # Reference PyTorch inference.
    with torch.no_grad():
        torch_ab = model(torch.from_numpy(input_np)).numpy()
    cv2.imwrite(str(args.image_out_dir / f"bw_pytorch_{args.name}.jpg"), postprocess(orig_l, torch_ab))
    print(f"pytorch ab range: [{torch_ab.min():.2f}, {torch_ab.max():.2f}]")

    # FP32 export, opset 17, fixed shape.
    print("exporting fp32 onnx...")
    torch.onnx.export(
        model,
        torch.zeros(1, 3, INPUT_SIZE, INPUT_SIZE),
        str(fp32_path),
        opset_version=17,
        input_names=["input"],
        output_names=["output"],
        do_constant_folding=True,
        dynamic_axes=None,
    )

    import onnx
    from onnxconverter_common import float16

    onnx.checker.check_model(str(fp32_path))

    # FP16 conversion with FP32 I/O edge nodes (project convention; Go always feeds float32).
    print("converting to fp16...")
    model_fp16 = float16.convert_float_to_float16(onnx.load(str(fp32_path)), keep_io_types=True)
    onnx.save(model_fp16, str(fp16_path))

    # Parity checks with onnxruntime.
    import onnxruntime as ort

    for path, tag in ((fp32_path, "fp32"), (fp16_path, "fp16")):
        sess = ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])
        ort_ab = sess.run(["output"], {"input": input_np})[0]
        stats(f"pytorch vs onnx_{tag}", torch_ab, ort_ab)
        cv2.imwrite(str(args.image_out_dir / f"bw_onnx_{args.name}_{tag}.jpg"), postprocess(orig_l, ort_ab))

    for path in (fp32_path, fp16_path):
        print(f"{path.name}: {path.stat().st_size / 1e6:.1f} MB  sha256={sha256(path)}")


if __name__ == "__main__":
    main()
