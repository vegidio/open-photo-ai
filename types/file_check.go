package types

// FileCheck represents a file validation configuration used to determine if a dependency needs to be downloaded.
//
// # Fields:
//   - Path: the path to the file within the destination directory to check for existence.
//   - Hash: the expected SHA256 hash of the file for integrity verification. If empty, no hash validation is performed.
type FileCheck struct {
	Path string
	Hash string
}
