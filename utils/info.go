package utils

import (
	"path/filepath"
	"slices"
	"strings"
)

// The extension lists are package-level so a membership test doesn't rebuild them. IsSupportedFile and IsRawExtension
// run once per dropped file and on every image load, and returning a fresh slice from each call meant allocating ~60
// strings per check. The exported accessors hand out clones so a caller can't mutate the shared backing arrays.
var (
	imageExtensions = []string{
		"avif",
		"bmp",
		"gif",
		"heic", "heif",
		"jpeg", "jpg",
		"png",
		"tif", "tiff",
		"webp",
	}

	rawExtensions = []string{
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

	// Lookup sets, so a membership test is a hash probe instead of a linear scan over ~60 entries.
	rawExtensionSet   = toSet(rawExtensions)
	inputExtensionSet = toSet(append(append([]string{}, imageExtensions...), rawExtensions...))
)

func toSet(exts []string) map[string]struct{} {
	set := make(map[string]struct{}, len(exts))
	for _, ext := range exts {
		set[ext] = struct{}{}
	}
	return set
}

// extensionOf returns path's lowercase extension without the leading dot.
func extensionOf(path string) string {
	ext := strings.ToLower(filepath.Ext(path))
	if len(ext) > 0 {
		ext = ext[1:]
	}
	return ext
}

// SupportedImageExtensions returns a list of image file extensions that the application can process.
//
// The returned extensions include common image formats that have registered decoders and encoders in this package. File
// extensions are returned in lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of supported image file extensions
func SupportedImageExtensions() []string {
	return slices.Clone(imageExtensions)
}

// SupportedRawExtensions returns a list of camera RAW file extensions that the application can read.
//
// RAW formats are read-only: they can be decoded (via github.com/vegidio/raw-go / LibRaw) but never written. File
// extensions are returned in lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of supported RAW file extensions
func SupportedRawExtensions() []string {
	return slices.Clone(rawExtensions)
}

// SupportedInputExtensions returns the union of standard image and RAW extensions accepted as input
// (file dialog filtering and drag-and-drop validation). Extensions are lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of all readable input file extensions
func SupportedInputExtensions() []string {
	return append(SupportedImageExtensions(), rawExtensions...)
}

// IsSupportedInputExtension reports whether path has an extension the app accepts as input (image or RAW).
//
// # Parameters:
//   - path: The file system path (or file name) to inspect
//
// # Returns:
//   - bool: true if the extension is a readable input format
func IsSupportedInputExtension(path string) bool {
	_, ok := inputExtensionSet[extensionOf(path)]
	return ok
}

// IsRawExtension reports whether path has a camera RAW file extension.
//
// # Parameters:
//   - path: The file system path (or file name) to inspect
//
// # Returns:
//   - bool: true if the extension is a supported RAW format
func IsRawExtension(path string) bool {
	_, ok := rawExtensionSet[extensionOf(path)]
	return ok
}
