package utils

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

// ClampFloat32 clamps a float32 value to [0, 255]
func ClampFloat32(val float32) float32 {
	if val < 0 {
		return 0
	}
	if val > 255 {
		return 255
	}
	return val
}

func Ceiling(val float64) float64 {
	if val > 0.999 {
		return 1.0
	}
	return val
}
