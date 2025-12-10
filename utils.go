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

// LoadImage loads an image file from the specified path and returns it as ImageData.
// It supports multiple image formats including JPEG, PNG, GIF, BMP, TIFF, and WebP.
//
// The function opens the file, decodes the image data, and constructs an ImageData
// structure containing both the file path and the decoded pixel data.
//
// # Parameters:
//   - path: The file system path to the image file to be loaded
//
// # Returns:
//   - *types.ImageData: A pointer to the ImageData structure containing the file path and decoded image
//   - error: An error if the file cannot be opened or the image cannot be decoded
func LoadImage(path string) (*types.ImageData, error) {
	inputFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer inputFile.Close()

	img, _, err := image.Decode(inputFile)
	if err != nil {
		return nil, err
	}

	return &types.ImageData{
		FilePath: path,
		Pixels:   img,
	}, nil
}

// SaveImage saves the image data to a file in the specified format.
// The output format is determined by the Format field in the OutputImage structure.
//
// For JPEG format, the quality parameter controls the compression level.
// For PNG format, the quality parameter is ignored as PNG uses lossless compression.
// For TIFF format, the quality parameter is ignored and Deflate compression is used.
//
// # Parameters:
//   - data: A pointer to OutputImage containing the file path, pixel data, and desired format.
//   - format: The image format to save the image data as.
//   - quality: The quality level the encoding (0-100, where 100 is the highest quality).
//
// # Returns:
//   - error: An error if the quality is out of range, the file cannot be created, or encoding fails
func SaveImage(data *types.ImageData, format types.ImageFormat, quality int) error {
	if quality < 0 || quality > 100 {
		return fmt.Errorf("invalid quality: %d, must be between 0 and 100", quality)
	}

	outputFile, err := os.Create(data.FilePath)
	if err != nil {
		return err
	}
	defer outputFile.Close()

	switch format {
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
