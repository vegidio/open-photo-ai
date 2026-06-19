package utils

import (
	"path/filepath"
	"slices"
	"strings"
)

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

// SupportedRawExtensions returns a list of camera RAW file extensions that the application can read.
//
// RAW formats are read-only: they can be decoded (via github.com/vegidio/raw-go / LibRaw) but never written. File
// extensions are returned in lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of supported RAW file extensions
func SupportedRawExtensions() []string {
	return []string{
		"crw", "cr2", "cr3", // Canon
		"nef", "nrw", // Nikon
		"arw", "srf", "sr2", // Sony
		"raf",               // Fujifilm
		"orf",               // Olympus
		"rw2", "raw", "rwl", // Panasonic/Leica
		"pef", "ptx", "dng", // Pentax/Ricoh (+ Adobe/generic DNG)
		"srw",        // Samsung
		"x3f",        // Sigma
		"3fr", "fff", // Hasselblad
		"iiq", "cap", "eip", // Phase One
		"dcr", "kdc", "k25", "dcs", "dc2", // Kodak
		"mos",        // Leaf
		"mef",        // Mamiya
		"mrw", "mdc", // Minolta
		"erf",                                                  // Epson
		"bay",                                                  // Casio
		"pxn",                                                  // Logitech
		"gpr",                                                  // GoPro
		"bmq", "cs1", "cine", "ia", "kc2", "qtk", "rdc", "sti", // misc
	}
}

// SupportedInputExtensions returns the union of standard image and RAW extensions accepted as input
// (file dialog filtering and drag-and-drop validation). Extensions are lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of all readable input file extensions
func SupportedInputExtensions() []string {
	return append(SupportedImageExtensions(), SupportedRawExtensions()...)
}

// IsRawExtension reports whether path has a camera RAW file extension.
//
// # Parameters:
//   - path: The file system path (or file name) to inspect
//
// # Returns:
//   - bool: true if the extension is a supported RAW format
func IsRawExtension(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))
	if len(ext) > 0 {
		ext = ext[1:]
	}
	return slices.Contains(SupportedRawExtensions(), ext)
}
