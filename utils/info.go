package utils

// SupportedImageExtensions returns a list of image file extensions that the application can process.
//
// The returned extensions include common image formats that have registered decoders and encoders in this package. File
// extensions are returned in lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of supported image file extensions
func SupportedImageExtensions() []string {
	return []string{
		"avif",
		"bmp",
		"gif",
		"heic", "heif",
		"jpeg", "jpg",
		"png",
		"tif", "tiff",
		"webp",
	}
}
