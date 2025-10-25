import torch
from basicsr.archs.rrdbnet_arch import RRDBNet

# ---- Configuration ----
model_path = "RealESRGAN_x4plus.pth"
output_path = "upscale_4x_general_fp32.onnx"
H, W = 256, 256 # input tile size
scale = 4       # Update to 2 if converting a 2x model

# ---- Build model ----
net = RRDBNet(
    num_in_ch=3,
    num_out_ch=3,
    num_feat=64,
    num_block=23,  # The cartoon model uses 6 blocks instead of 23
    num_grow_ch=32,
    scale=scale
)

# ---- Load pretrained weights ----
ckpt = torch.load(model_path, map_location="cpu")
net.load_state_dict(ckpt["params_ema" if "params_ema" in ckpt else "params"], strict=True)
net.eval()

# ---- Export ----
dummy = torch.randn(1, 3, H, W)
torch.onnx.export(
    net, dummy, output_path,
    input_names=["input"], output_names=["output"],
    opset_version=16,
    do_constant_folding=True,
    dynamic_axes=None  # ensures fixed shape
)

print(f"✅ Exported to {output_path}")