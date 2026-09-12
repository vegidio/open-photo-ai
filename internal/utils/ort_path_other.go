//go:build !windows

package utils

// ortCachePath is the identity outside Windows: macOS and Linux both use UTF-8 as the narrow encoding, so the bytes
// ONNX Runtime receives are the bytes Go sent and any path it can be given is one it can open. See the Windows build
// of this file for what it is working around there.
func ortCachePath(dir string) (string, bool) {
	return dir, true
}
