package opai

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

// LoadInputImage loads an image file from the specified path and returns it as InputImage.
// It supports multiple image formats including JPEG, PNG, GIF, BMP, TIFF, and WebP.
//
// The function opens the file, decodes the image data, and constructs an InputImage
// structure containing both the file path and the decoded pixel data.
//
// # Parameters:
//   - path: The file system path to the image file to be loaded
//
// # Returns:
//   - *types.InputImage: A pointer to the InputImage structure containing the file path and decoded image
//   - error: An error if the file cannot be opened or the image cannot be decoded
func LoadInputImage(path string) (*types.InputImage, error) {
	inputFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer inputFile.Close()

	img, _, err := image.Decode(inputFile)
	if err != nil {
		return nil, err
	}

	return &types.InputImage{
		FilePath: path,
		Pixels:   img,
	}, nil
}

// SaveOutputImage saves the image data to a file in the specified format.
// The output format is determined by the Format field in the OutputImage structure.
// Supported formats include JPEG, PNG, and TIFF.
//
// For JPEG format, the quality parameter controls the compression level.
// For PNG format, the quality parameter is ignored as PNG uses lossless compression.
// For TIFF format, the quality parameter is ignored and Deflate compression is used.
//
// # Parameters:
//   - data: A pointer to OutputImage containing the file path, pixel data, and desired format
//   - quality: The quality level for JPEG encoding (0-100, where 100 is the highest quality)
//
// # Returns:
//   - error: An error if the quality is out of range, the file cannot be created, or encoding fails
func SaveOutputImage(data *types.OutputImage, quality int) error {
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
