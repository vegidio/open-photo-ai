package types

// FileCheck decides whether a dependency needs downloading: Path is relative to the destination directory, and Hash
// is the expected SHA-256 of the file. An empty Hash skips verification entirely.
type FileCheck struct {
	Path string
	Hash string
}
