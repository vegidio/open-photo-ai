package types

import "image"

// InputImage represents an image file loaded into memory for processing.
// It contains both the file system path and the decoded pixel data of the image.
//
// # Fields:
//   - FilePath: The file system path from which the image was loaded
//   - Pixels: The decoded image data as an image.Image interface
type InputImage struct {
	FilePath string
	Pixels   image.Image
}

// OutputImage represents an image ready to be saved to disk.
// It contains the destination file path, the processed pixel data, and the desired output format.
//
// # Fields:
//   - FilePath: The file system path where the image will be saved
//   - Pixels: The processed image data as an image.Image interface
//   - Format: The desired output image format (e.g., JPEG, PNG, TIFF)
type OutputImage struct {
	FilePath string
	Pixels   image.Image
	Format   ImageFormat
}

// ImageFormat describes the type of image used.
type ImageFormat int

// Constants for supported image formats.
const (
	FormatJpeg ImageFormat = iota
	FormatPng
	FormatTiff
)
