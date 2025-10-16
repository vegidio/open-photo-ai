package openphotoai

import (
	"image"
	_ "image/gif"
	"image/jpeg"
	_ "image/jpeg"
	"image/png"
	_ "image/png"
	"os"

	_ "golang.org/x/image/bmp"
	_ "golang.org/x/image/tiff"
	_ "golang.org/x/image/webp"

	"github.com/vegidio/open-photo-ai/internal/types"
)

func LoadInputData(path string) (*types.InputData, error) {
	inputFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer inputFile.Close()

	img, _, err := image.Decode(inputFile)
	if err != nil {
		return nil, err
	}

	return &types.InputData{
		FilePath: path,
		Pixels:   img,
	}, nil
}

func SaveOutputData(data *types.OutputData) error {
	outputFile, err := os.Create(data.FilePath)
	if err != nil {
		return err
	}
	defer outputFile.Close()

	switch data.Format {
	case "png":
		err = png.Encode(outputFile, data.Pixels)
	case "jpg":
		err = jpeg.Encode(outputFile, data.Pixels, nil)
	}

	if err != nil {
		return err
	}

	return nil
}
