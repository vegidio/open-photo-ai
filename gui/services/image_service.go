package services

import (
	"bytes"
	"image/jpeg"

	opai "github.com/vegidio/open-photo-ai"
)

type ImageService struct{}

func (i *ImageService) GetImageBytes(filePath string) ([]byte, error) {
	data, err := opai.LoadInputData(filePath)
	if err != nil {
		return nil, err
	}

	var buf bytes.Buffer
	if err = jpeg.Encode(&buf, data.Pixels, &jpeg.Options{Quality: 100}); err != nil {
		return nil, err
	}

	return buf.Bytes(), nil
}
