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
//
// The values are strings rather than an iota so that the zero value is the empty string - an unusable format that
// EncodeImage rejects - instead of silently being the first format in the list, and so that inserting a format never
// renumbers the ones after it. The values double as the file extension each format is written with.
type ImageFormat string

const (
	// FormatAvif is the AV1 Image File Format, a lossy format with very high compression.
	FormatAvif ImageFormat = "avif"

	// FormatBmp is the Windows bitmap format, uncompressed.
	FormatBmp ImageFormat = "bmp"

	// FormatGif is the Graphics Interchange Format, limited to a 256-colour palette.
	FormatGif ImageFormat = "gif"

	// FormatHeic is the High Efficiency Image Container, a lossy format used by Apple devices.
	FormatHeic ImageFormat = "heic"

	// FormatJpeg is the JPEG format, the most widely supported lossy format.
	FormatJpeg ImageFormat = "jpeg"

	// FormatPng is the Portable Network Graphics format, lossless with alpha support.
	FormatPng ImageFormat = "png"

	// FormatTiff is the Tagged Image File Format, lossless and the fallback for inputs that cannot be re-encoded.
	FormatTiff ImageFormat = "tiff"

	// FormatWebp is Google's WebP format, lossy with better compression than JPEG.
	FormatWebp ImageFormat = "webp"
)
