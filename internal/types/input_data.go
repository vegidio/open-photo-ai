package types

import "image"

type InputData struct {
	FilePath string
	Pixels   image.Image
	Exif     map[string]string
}
