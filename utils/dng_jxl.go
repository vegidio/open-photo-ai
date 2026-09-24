package utils

import (
	"bytes"
	"image"

	"github.com/gen2brain/jpegxl"
)

// decodeJXL is the one place the JPEG XL codec is called, so it can be swapped without touching the DNG handling.
// jpegxl uses the system's libjxl when there is one and a bundled WebAssembly build otherwise.
var decodeJXL = func(data []byte) (image.Image, error) {
	return jpegxl.Decode(bytes.NewReader(data))
}
