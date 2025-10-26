import onnx

path_in = "upscale_general_dyn_4x_fp32.onnx"
path_out = "upscale_general_1024_4x_fp32.onnx"

# Desired fixed shapes per input name
fixed = {
    "input": [1, 3, 256, 256],
}

m = onnx.load(path_in)

# Fill shapes for inputs
for inp in m.graph.input:
    name = inp.name
    if name in fixed:
        dims = fixed[name]
        shape = inp.type.tensor_type.shape
        for i, d in enumerate(shape.dim):
            d.dim_value = int(dims[i])

# (Optional) Do the same for outputs if they also carry dynamic dims
for out in m.graph.output:
    shape = out.type.tensor_type.shape
    for d in shape.dim:
        if not d.HasField("dim_value"):
            d.dim_value = 1  # pick what you need

onnx.save(m, path_out)
print("Saved:", path_out)