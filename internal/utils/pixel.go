package utils

import "image"

// rgbPixBuffer returns the backing pixel slice and row stride for the concrete RGBA-family image
// types, or ok=false for any other implementation. The returned offsets are relative to Bounds().Min.
func rgbPixBuffer(img image.Image) (pix []uint8, stride int, ok bool) {
	switch m := img.(type) {
	case *image.NRGBA:
		return m.Pix, m.Stride, true
	case *image.RGBA:
		return m.Pix, m.Stride, true
	default:
		return nil, 0, false
	}
}

// sample16 returns the 16-bit (0-65535) RGBA channel values for the pixel at byte offset off in a
// concrete RGBA-family buffer, matching exactly what color.RGBA/NRGBA.RGBA() would return. isNRGBA
// must be true for *image.NRGBA buffers (straight alpha) and false for premultiplied *image.RGBA.
func sample16(pix []uint8, off int, isNRGBA bool) (r, g, b, a uint32) {
	r = uint32(pix[off]) * 257
	g = uint32(pix[off+1]) * 257
	b = uint32(pix[off+2]) * 257
	av := uint32(pix[off+3])
	a = av * 257

	if isNRGBA && av != 0xff {
		r = r * av / 0xff
		g = g * av / 0xff
		b = b * av / 0xff
	}

	return
}
