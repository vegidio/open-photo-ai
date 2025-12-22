package utils

// LibVersion returns the current version of the library.
//
// The version follows calendar versioning format (YY.MM.PATCH) and represents the release version of the open-photo-ai
// library.
//
// # Returns:
//   - string: The current version as string.
func LibVersion() string {
	// Placeholder, replaced during the build pipeline
	return "<version>"
}

// SupportedImageExtensions returns a list of image file extensions that the application can process.
//
// The returned extensions include common image formats that have registered decoders and encoders
// in this package. File extensions are returned in lowercase without the leading dot.
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
