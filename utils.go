package openphotoai

import (
	"fmt"
	"image"
	_ "image/gif"
	"image/jpeg"
	_ "image/jpeg"
	"image/png"
	_ "image/png"
	"os"

	"github.com/vegidio/open-photo-ai/types"
	_ "golang.org/x/image/bmp"
	"golang.org/x/image/tiff"
	_ "golang.org/x/image/tiff"
	_ "golang.org/x/image/webp"
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

func SaveOutputData(data *types.OutputData, quality int) error {
	if quality < 0 || quality > 100 {
		return fmt.Errorf("invalid quality: %d, must be between 0 and 100", quality)
	}

	outputFile, err := os.Create(data.FilePath)
	if err != nil {
		return err
	}
	defer outputFile.Close()

	switch data.Format {
	case types.FormatJpeg:
		err = jpeg.Encode(outputFile, data.Pixels, &jpeg.Options{Quality: quality})
	case types.FormatPng:
		err = png.Encode(outputFile, data.Pixels)
	case types.FormatTiff:
		err = tiff.Encode(outputFile, data.Pixels, &tiff.Options{Compression: tiff.Deflate})
	}

	if err != nil {
		return err
	}

	return nil
}
