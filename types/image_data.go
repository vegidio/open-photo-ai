package types

import "image"

// ImageData represents an image file loaded into memory for processing.
// It contains both the file system path and the decoded pixel data of the image.
//
// # Fields:
//   - FilePath: The file system path from which the image was loaded
//   - Pixels: The decoded image data as an image.Image interface
type ImageData struct {
	FilePath string
	Pixels   image.Image
	Hash     string
}

// ImageFormat describes the type of image used.
type ImageFormat int

// Constants for supported image formats.
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
