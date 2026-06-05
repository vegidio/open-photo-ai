package utils

import "math"

// RgbToHsv converts an RGB color to HSV (Hue, Saturation, Value). All input channels (r, g, b) are expected to be in
// the range [0, 255].
//
// The returned values are:
//   - h: hue in degrees, in the range [0, 360)
//   - s: saturation, scaled to the range [0, 255]
//   - v: value (brightness), in the range [0, 255]
func RgbToHsv(r, g, b float64) (h, s, v float64) {
	maxC := math.Max(r, math.Max(g, b))
	minC := math.Min(r, math.Min(g, b))
	delta := maxC - minC

	v = maxC

	if maxC == 0 {
		s = 0
	} else {
		s = (delta / maxC) * 255.0
	}

	if delta == 0 {
		h = 0
	} else {
		switch maxC {
		case r:
			h = 60.0 * math.Mod((g-b)/delta, 6.0)
		case g:
			h = 60.0 * (((b - r) / delta) + 2.0)
		default:
			h = 60.0 * (((r - g) / delta) + 4.0)
		}
	}

	if h < 0 {
		h += 360.0
	}

	return
}
