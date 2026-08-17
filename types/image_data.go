package types

import "image"

// ImageData is an image file loaded into memory for processing.
type ImageData struct {
	FilePath string
	Pixels   image.Image

	// Hash identifies the source pixels and keys the per-operation image cache. utils.LoadImage fills it with the
	// xxh3 of the file's bytes; a caller that alters Pixels afterwards must alter Hash too, or the cache returns a
	// result computed from the original image.
	Hash string
}

// ImageFormat describes the type of image used.
type ImageFormat int

const (
	FormatAvif ImageFormat = iota
	FormatBmp
	FormatGif
	FormatHeic
	FormatJpeg
	FormatPng
	FormatTiff
	FormatWebp
)
