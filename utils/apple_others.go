//go:build !darwin

package utils

// IsCoreMLSupported always returns false on non-macOS platforms.
func IsCoreMLSupported() bool {
	return false
}
