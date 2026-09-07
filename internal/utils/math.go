package utils

import "math"

func ClampInt(val, minVal, maxVal int) int {
	if val < minVal {
		return minVal
	}
	if val > maxVal {
		return maxVal
	}
	return val
}

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

// FitLongSide returns (newW, newH) scaled so the longest side is exactly size, preserving the aspect ratio and
// rounding to nothing else.
//
// It is the half of FitToMaxSize a fixed-shape graph wants on its own: rounding up to 16 would overshoot the one
// dimension such a graph accepts. Neither side is ever returned as zero - an aspect ratio extreme enough to round the
// short side away still has to produce an image.
func FitLongSide(w, h, size int) (int, int) {
	ratio := float64(size) / float64(max(w, h))
	nw := max(1, int(math.Round(float64(w)*ratio)))
	nh := max(1, int(math.Round(float64(h)*ratio)))

	return nw, nh
}

// FitToMaxSize returns (newW, newH) such that the longest side equals maxSize and both dimensions are rounded up to the
// next multiple of 16.
func FitToMaxSize(w, h, maxSize int) (int, int) {
	nw, nh := FitLongSide(w, h, maxSize)
	return RoundUpTo16(nw), RoundUpTo16(nh)
}

// FitWithinMaxSize returns (newW, newH) with neither side longer than maxSize and both rounded up to the next multiple
// of 16.
//
// It differs from FitToMaxSize in that it never enlarges: an image already inside the ceiling keeps its own dimensions
// and is only aligned. Running a small image at the ceiling costs inference time proportional to the area it was
// stretched to while adding no detail the source did not have.
func FitWithinMaxSize(w, h, maxSize int) (int, int) {
	if w <= maxSize && h <= maxSize {
		return RoundUpTo16(w), RoundUpTo16(h)
	}

	return FitToMaxSize(w, h, maxSize)
}

func RoundUpTo16(v int) int {
	if v%16 == 0 {
		return v
	}
	return v + (16 - v%16)
}
