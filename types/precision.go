package types

import "github.com/cockroachdb/errors"

// Precision is the numeric format a model's weights are published in. It is part of a model's identity: it appears in
// the operation id, in the downloaded file name, and in the engine cache directory compiled from it.
type Precision string

const (
	PrecisionFp32 Precision = "fp32"
	PrecisionFp16 Precision = "fp16"

	// PrecisionInt8 is a weight-only quantized build: the weights are stored as int8 and the activations are not
	// quantized at all. It is not a third tier every model has - only a model whose weights dominate its download is
	// worth publishing this way, and only where the quantization was measured to be visually lossless.
	PrecisionInt8 Precision = "int8"
)

// Valid reports whether p is one of the published precisions.
func (p Precision) Valid() bool {
	switch p {
	case PrecisionFp32, PrecisionFp16, PrecisionInt8:
		return true
	default:
		return false
	}
}

// ParsePrecision converts an untrusted string into a Precision, rejecting anything that is not a published one.
//
// A Precision is a bare string that ends up in a filesystem path - internal.EngineCacheFor composes the engine cache
// directory from it, and that directory is later emptied wholesale - and in a download URL. Anything that builds one
// from input it did not produce itself parses it here rather than converting it, so a segment like "../.." is refused
// at the boundary instead of reaching those paths.
func ParsePrecision(s string) (Precision, error) {
	p := Precision(s)
	if !p.Valid() {
		return "", errors.Newf("unknown precision %q", s)
	}

	return p, nil
}
