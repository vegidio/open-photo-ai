package types

import "image"

type OutputData struct {
	FilePath string
	Pixels   image.Image
	Exif     map[string]string
	Format   string
}
