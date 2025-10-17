package types

import "image"

type InputData struct {
	FilePath string
	Pixels   image.Image
}

type OutputData struct {
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
