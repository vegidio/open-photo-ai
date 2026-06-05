package utils

import "math"

// ClampInt clamps an integer value between minVal and maxVal
func ClampInt(val, minVal, maxVal int) int {
	if val < minVal {
		return minVal
	}
	if val > maxVal {
		return maxVal
	}
	return val
}

// Clamp255 clamps a float32 value to the [0, 255] byte range (callers typically cast the result to uint8).
func Clamp255(val float32) float32 {
	if val < 0 {
		return 0
	}
	if val > 255 {
		return 255
	}
	return val
}

// ClampProgress snaps a near-complete progress value (>0.999) to exactly 1.0, so the final
// onProgress callback reports 100% instead of e.g. 0.9998.
func ClampProgress(val float64) float64 {
	if val > 0.999 {
		return 1.0
	}
	return val
}

// FitToMaxSize returns (newW, newH) such that the longest side equals maxSize and both dimensions are rounded up to the
// next multiple of 16.
func FitToMaxSize(w, h, maxSize int) (int, int) {
	longest := w
	if h > longest {
		longest = h
	}
	ratio := float64(maxSize) / float64(longest)
	nw := int(math.Round(float64(w) * ratio))
	nh := int(math.Round(float64(h) * ratio))
	return RoundUpTo16(nw), RoundUpTo16(nh)
}

// RoundUpTo16 rounds v up to the next multiple of 16.
func RoundUpTo16(v int) int {
	if v%16 == 0 {
		return v
	}
	return v + (16 - v%16)
}
