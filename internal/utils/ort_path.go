package utils

// isASCIIPath reports whether every byte of path is ASCII.
//
// It is a byte scan rather than a range over runes because what matters is the encoding boundary, not the characters:
// ONNX Runtime takes the provider cache directories as narrow C strings, and any byte at or above 0x80 is one the
// receiving end may decode differently from how Go wrote it. See ortCachePath.
func isASCIIPath(path string) bool {
	for i := range len(path) {
		if path[i] >= 0x80 {
			return false
		}
	}

	return true
}
