import onnx
from onnxconverter_common import float16
from onnx import shape_inference

model = onnx.load("upscale_general_4x_fp32.onnx")
model_fp16 = float16.convert_float_to_float16(model, keep_io_types=True)

# Either re-infer shapes…
model_fp16 = shape_inference.infer_shapes(model_fp16)

# …or, if that still leaves mismatches, clear internal value_info entirely:
model_fp16.graph.ClearField("value_info")

onnx.save(model_fp16, "upscale_general_4x_fp16.onnx")