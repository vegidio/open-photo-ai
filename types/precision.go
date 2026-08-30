package types

type Precision string

const (
	PrecisionFp32 Precision = "fp32"
	PrecisionFp16 Precision = "fp16"

	// PrecisionInt8 is a weight-only quantized build: the weights are stored as int8 and the activations are not
	// quantized at all. It is not a third tier every model has - only a model whose weights dominate its download is
	// worth publishing this way, and only where the quantization was measured to be visually lossless.
	PrecisionInt8 Precision = "int8"
)
