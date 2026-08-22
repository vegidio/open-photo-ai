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

// CIELab support (sRGB, D65). The constants and piecewise thresholds follow OpenCV's float32 Lab conversion, which is
// the convention colorization models are trained against: L in [0, 100] and a/b zero-centered (not the 8-bit encoding
// that scales L by 255/100 and offsets a/b by 128).
const (
	labXn = 0.950456 // D65 white point, X
	labZn = 1.088754 // D65 white point, Z
	labT0 = 0.008856 // (6/29)^3, linear/cubic threshold
	labK  = 903.3    // 29^3/3^3, low-luminance L slope
)

// RgbToLab converts an sRGB color to CIELab (D65). Inputs r, g, b are in [0, 1]; the returned l is in [0, 100] and
// a, b are zero-centered (roughly [-128, 127]).
func RgbToLab(r, g, b float32) (l, la, lb float32) {
	lr := srgbToLinear(float64(r))
	lg := srgbToLinear(float64(g))
	lb64 := srgbToLinear(float64(b))

	x := (0.412453*lr + 0.357580*lg + 0.180423*lb64) / labXn
	y := 0.212671*lr + 0.715160*lg + 0.072169*lb64
	z := (0.019334*lr + 0.119193*lg + 0.950227*lb64) / labZn

	fx := labF(x)
	fy := labF(y)
	fz := labF(z)

	var lum float64
	if y > labT0 {
		lum = 116.0*fy - 16.0
	} else {
		lum = labK * y
	}

	return float32(lum), float32(500.0 * (fx - fy)), float32(200.0 * (fy - fz))
}

// LabToRgb converts a CIELab color (D65, the same convention as RgbToLab) back to sRGB. The returned channels are
// clamped to [0, 1].
func LabToRgb(l, la, lb float32) (r, g, b float32) {
	l64 := float64(l)

	var y, fy float64
	if l64 > labK*labT0 {
		fy = (l64 + 16.0) / 116.0
		y = fy * fy * fy
	} else {
		y = l64 / labK
		fy = 7.787*y + 16.0/116.0
	}

	x := labXn * labFInv(fy+float64(la)/500.0)
	z := labZn * labFInv(fy-float64(lb)/200.0)

	lr := 3.240479*x - 1.537150*y - 0.498535*z
	lg := -0.969256*x + 1.875992*y + 0.041556*z
	lb64 := 0.055648*x - 0.204043*y + 1.057311*z

	return float32(linearToSrgb(lr)), float32(linearToSrgb(lg)), float32(linearToSrgb(lb64))
}

// labF is the CIE f(t) forward transfer with OpenCV's linear-segment approximation below the threshold.
func labF(t float64) float64 {
	if t > labT0 {
		return math.Cbrt(t)
	}
	return 7.787*t + 16.0/116.0
}

// labFInv inverts labF.
func labFInv(ft float64) float64 {
	if t := ft * ft * ft; t > labT0 {
		return t
	}
	return (ft - 16.0/116.0) / 7.787
}

// srgbToLinear removes the sRGB gamma from a [0, 1] channel.
func srgbToLinear(c float64) float64 {
	if c <= 0.04045 {
		return c / 12.92
	}
	return math.Pow((c+0.055)/1.055, 2.4)
}

// linearToSrgb applies the sRGB gamma to a linear-light channel and clamps to [0, 1].
func linearToSrgb(c float64) float64 {
	if c <= 0 {
		return 0
	}
	if c <= 0.0031308 {
		c *= 12.92
	} else {
		c = 1.055*math.Pow(c, 1.0/2.4) - 0.055
	}
	return math.Min(c, 1)
}

// region - Fast paths

// The colorization pipeline runs these conversions once per pixel at full photo resolution, where the transcendental
// calls dominate. Two properties make them avoidable without changing a single output byte:
//
//  1. Only L is ever needed from the forward conversion at full resolution, and L depends solely on Y - so two of the
//     three cube roots and both chroma terms are dead work.
//  2. Every channel that reaches the forward path comes from an 8-bit pixel byte via Sample16, so srgbToLinear has
//     only 256 distinct arguments; and the reverse path's only consumer is a rounded 8-bit channel, so it has only
//     256 distinct results.
//
// Both tables are built to agree with the general path exactly, and TestSrgbByteMatchesPow pins that agreement at
// every step boundary - which, for a monotonic transfer, pins it everywhere.

// srgbLinearLUT[v] is srgbToLinear for the 8-bit channel value v exactly as it arrives through Sample16, which scales
// a byte to 16-bit by *257; the caller then divides by 65535 in float32.
var srgbLinearLUT [256]float64

// srgbByteThreshold[k] is the smallest linear-light value that encodes to output byte k+1. Because the transfer is
// monotonic, counting the thresholds at or below a value yields the same byte the math.Pow expression would.
var srgbByteThreshold [255]float64

func init() {
	for v := range 256 {
		srgbLinearLUT[v] = srgbToLinear(float64(float32(uint32(v)*257) / 65535.0))
	}

	// Locate each step boundary by bisecting the float64 bit pattern rather than the value: for positive floats the
	// bit order is the value order, so this converges on the exact float64 where the output byte changes.
	for k := 1; k <= 255; k++ {
		lo, hi := math.Float64bits(0), math.Float64bits(1)

		for lo < hi {
			mid := lo + (hi-lo)/2
			if srgbByteSlow(math.Float64frombits(mid)) >= uint8(k) {
				hi = mid
			} else {
				lo = mid + 1
			}
		}

		srgbByteThreshold[k-1] = math.Float64frombits(lo)
	}
}

// RgbToLabL returns only the CIELab L of an sRGB color, bit-for-bit identical to the first result of RgbToLab. Inputs
// are in [0, 1]; the result is in [0, 100].
func RgbToLabL(r, g, b float32) float32 {
	return labLFromLinear(srgbToLinear(float64(r)), srgbToLinear(float64(g)), srgbToLinear(float64(b)))
}

// RgbToLabLBytes is RgbToLabL for channels that came straight from 8-bit pixel bytes, taking the linearization from a
// lookup table instead of math.Pow. Valid only where Sample16 returns v*257 unscaled - that is, for premultiplied
// buffers and for straight-alpha buffers at full opacity; callers must use RgbToLabL otherwise.
func RgbToLabLBytes(r, g, b uint8) float32 {
	return labLFromLinear(srgbLinearLUT[r], srgbLinearLUT[g], srgbLinearLUT[b])
}

// labLFromLinear is the L half of the CIELab forward transfer, shared by both entry points above.
func labLFromLinear(lr, lg, lb float64) float32 {
	y := 0.212671*lr + 0.715160*lg + 0.072169*lb

	if y > labT0 {
		return float32(116.0*labF(y) - 16.0)
	}

	return float32(labK * y)
}

// LabToLinearRgb is LabToRgb stopping one step short: it returns linear-light channels, unclamped and still in
// float64, for callers that finish with SrgbByte instead of the float32 gamma encode.
func LabToLinearRgb(l, la, lb float32) (r, g, b float64) {
	l64 := float64(l)

	var y, fy float64
	if l64 > labK*labT0 {
		fy = (l64 + 16.0) / 116.0
		y = fy * fy * fy
	} else {
		y = l64 / labK
		fy = 7.787*y + 16.0/116.0
	}

	x := labXn * labFInv(fy+float64(la)/500.0)
	z := labZn * labFInv(fy-float64(lb)/200.0)

	return 3.240479*x - 1.537150*y - 0.498535*z,
		-0.969256*x + 1.875992*y + 0.041556*z,
		0.055648*x - 0.204043*y + 1.057311*z
}

// SrgbByte gamma-encodes a linear-light channel straight to the rounded 8-bit value it would end up as, replacing the
// math.Pow in linearToSrgb with a search over 255 precomputed thresholds.
func SrgbByte(c float64) uint8 {
	lo, hi := 0, 255

	for lo < hi {
		mid := (lo + hi) / 2
		if srgbByteThreshold[mid] <= c {
			lo = mid + 1
		} else {
			hi = mid
		}
	}

	return uint8(lo)
}

// srgbByteSlow is the reference the threshold table is built and tested against: the exact expression the compose loop
// used before the table existed.
func srgbByteSlow(c float64) uint8 {
	return uint8(Clamp255(float32(linearToSrgb(c))*255.0 + 0.5))
}

// endregion
