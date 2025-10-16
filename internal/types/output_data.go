package types

import "image"

type OuputData struct {
	FilePath string
	Pixels   image.Image
	Exif     map[string]string
}
