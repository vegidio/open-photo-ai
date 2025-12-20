package utils

import (
	"bytes"
	"fmt"
	"image"
	"image/gif"
	_ "image/gif"
	"image/jpeg"
	_ "image/jpeg"
	"image/png"
	_ "image/png"
	"os"

	"github.com/vegidio/open-photo-ai/types"
	"golang.org/x/image/bmp"
	_ "golang.org/x/image/bmp"
	"golang.org/x/image/tiff"
	_ "golang.org/x/image/tiff"
)

// SupportedImageExtensions returns a list of image file extensions that the application can process.
//
// The returned extensions include common image formats that have registered decoders and encoders
// in this package. File extensions are returned in lowercase without the leading dot.
//
// # Returns:
//   - []string: A slice of supported image file extensions
func SupportedImageExtensions() []string {
	return []string{
		"bmp",
		"gif",
		"jpeg", "jpg",
		"png",
		"tif", "tiff",
	}
}

// LoadImage loads an image file from the specified path and returns it as ImageData.
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

// EncodeImage encodes an image into a byte array in the specified format with the given quality level.
//
// # Parameters:
//   - img: The image.Image to be encoded.
//   - format: The target image format (e.g., FormatBmp, FormatJpeg, FormatPng).
//   - quality: The quality level for lossy formats like JPEG (0-100, where 100 is the highest quality).
//
// # Returns:
//   - []byte: The encoded image data as a byte slice.
//   - error: An error if the format is unsupported or encoding fails.
func EncodeImage(img image.Image, format types.ImageFormat, quality int) ([]byte, error) {
	var buf bytes.Buffer
	var err error

	switch format {
	case types.FormatBmp:
		err = bmp.Encode(&buf, img)
	case types.FormatGif:
		err = gif.Encode(&buf, img, &gif.Options{NumColors: 256})
	case types.FormatJpeg:
		err = jpeg.Encode(&buf, img, &jpeg.Options{Quality: quality})
	case types.FormatPng:
		encoder := &png.Encoder{CompressionLevel: png.DefaultCompression}
		err = encoder.Encode(&buf, img)
	case types.FormatTiff:
		err = tiff.Encode(&buf, img, &tiff.Options{Compression: tiff.Deflate})
	default:
		err = fmt.Errorf("unsupported image format: %d", format)
	}

	if err != nil {
		return nil, err
	}

	return buf.Bytes(), nil
}

// SaveImage saves the image data to a file in the specified format. The output format is determined by the Format field
// in the OutputImage structure.
//
// # Parameters:
//   - data: A pointer to OutputImage containing the file path, pixel data, and desired format.
//   - format: The image format to save the image data as.
//   - quality: The quality level the encoding (0-100, where 100 is the highest quality).
//
// # Returns:
//   - int64: The size of the saved file in bytes.
//   - error: An error if the quality is out of range, the file cannot be created, or encoding fails.
func SaveImage(data *types.ImageData, format types.ImageFormat, quality int) (int, error) {
	if quality < 0 || quality > 100 {
		return 0, fmt.Errorf("invalid quality: %d, must be between 0 and 100", quality)
	}

	imageBytes, err := EncodeImage(data.Pixels, format, quality)
	if err != nil {
		return 0, err
	}

	err = os.WriteFile(data.FilePath, imageBytes, 0644)
	if err != nil {
		return 0, err
	}

	return len(imageBytes), nil
}
