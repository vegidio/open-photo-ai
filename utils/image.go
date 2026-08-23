package utils

import (
	"bytes"
	"image"
	"image/gif"
	_ "image/gif"
	"image/jpeg"
	_ "image/jpeg"
	"image/png"
	_ "image/png"
	"io"
	"os"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/avif-go"
	_ "github.com/vegidio/avif-go"
	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/heif-go"
	_ "github.com/vegidio/heif-go"
	"github.com/vegidio/raw-go"
	"github.com/vegidio/webp-go"
	_ "github.com/vegidio/webp-go"

	"github.com/vegidio/open-photo-ai/types"
	"golang.org/x/image/bmp"
	_ "golang.org/x/image/bmp"
	"golang.org/x/image/tiff"
	_ "golang.org/x/image/tiff"
)

// rawMu serializes all calls into raw-go (LibRaw). The bundled LibRaw is not safe for concurrent
// decoding — running multiple decodes in parallel corrupts their data (LibRaw prints "data corrupted
// at …" and may return an all-black image), so every RAW decode/config call must hold this lock.
var rawMu sync.Mutex

// avifAlphaQuality is the quality of the AVIF alpha channel. Deliberately fixed rather than following the caller's
// quality: alpha is a coverage mask, not picture detail, and degrading it shows up as haloing around transparent
// edges long before the colour channels look wrong.
const avifAlphaQuality = 60

// LoadImage decodes an image file into memory, filling ImageData.Hash with the xxh3 of the file's bytes as it goes.
// Camera RAW files are decoded through LibRaw; see IsRawExtension for the formats that covers.
func LoadImage(path string) (*types.ImageData, error) {
	inputFile, err := os.Open(path)
	if err != nil {
		return nil, errors.Wrap(err, "failed to open image file")
	}
	defer inputFile.Close()

	// Hash the file as it is decoded rather than rewinding and reading it a second time — for a 60 MB RAW that second
	// pass is not free. The hash keys the image cache and has to match crypto.Xxh3File for the same file, so it must
	// still cover every byte: decoders routinely stop before EOF (trailing metadata, padding), hence the drain below.
	pipeReader, pipeWriter := io.Pipe()
	hashed := make(chan struct{})

	var hash string
	var hashErr error

	go func() {
		defer close(hashed)
		hash, hashErr = crypto.Xxh3Reader(pipeReader)
		// Keep consuming after the hasher is done so a decoder that stops early can never block writing into the pipe.
		_, _ = io.Copy(io.Discard, pipeReader)
	}()

	source := io.TeeReader(inputFile, pipeWriter)

	// RAW formats (except CR2/RAF) share the plain TIFF magic, so image.Decode can't be relied on to route them
	// correctly — branch on the extension and decode them explicitly with raw-go.
	var img image.Image
	if IsRawExtension(path) {
		rawMu.Lock()
		img, err = raw.Decode(source)
		rawMu.Unlock()
	} else {
		img, _, err = image.Decode(source)
	}

	// Feed the hasher whatever the decoder left behind, then close the pipe so it sees EOF.
	var drainErr error
	if err == nil {
		_, drainErr = io.Copy(io.Discard, source)
	}

	pipeWriter.Close()
	<-hashed

	if err != nil {
		return nil, errors.Wrap(err, "failed to decode image")
	}
	if drainErr != nil {
		return nil, errors.Wrap(drainErr, "failed to read image file")
	}
	if hashErr != nil {
		return nil, errors.Wrap(hashErr, "failed to compute image hash")
	}

	return &types.ImageData{
		FilePath: path,
		Pixels:   img,
		Hash:     hash,
	}, nil
}

// ImageDimensions returns {width, height} from an image file's header, without decoding the pixels. For RAW files it
// parses the metadata via raw-go (no demosaicing), which centralizes the RAW-aware logic so callers don't depend on
// raw-go directly.
func ImageDimensions(path string) ([]int, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, errors.Wrap(err, "failed to open image file")
	}
	defer file.Close()

	var config image.Config
	if IsRawExtension(path) {
		rawMu.Lock()
		config, err = raw.DecodeConfig(file)
		rawMu.Unlock()
	} else {
		config, _, err = image.DecodeConfig(file)
	}
	if err != nil {
		return nil, errors.Wrap(err, "failed to decode image config")
	}

	return []int{config.Width, config.Height}, nil
}

// EncodeImage encodes an image into the given format.
//
// quality is 0-100 and applies to the lossy formats - AVIF, HEIC, JPEG and WebP. The lossless ones (BMP, GIF, PNG,
// TIFF) ignore it. The scales are not comparable across encoders: 60 in libheif is not 60 in libjpeg, which is why
// each format carries its own default in the GUI rather than sharing one.
func EncodeImage(img image.Image, format types.ImageFormat, quality int) ([]byte, error) {
	var buf bytes.Buffer
	var err error

	switch format {
	case types.FormatAvif:
		err = avif.Encode(&buf, img, &avif.Options{Speed: 6, AlphaQuality: avifAlphaQuality, ColorQuality: quality})
	case types.FormatBmp:
		err = bmp.Encode(&buf, img)
	case types.FormatGif:
		err = gif.Encode(&buf, img, &gif.Options{NumColors: 256})
	case types.FormatHeic:
		err = heif.Encode(&buf, img, &heif.Options{Quality: quality})
	case types.FormatJpeg:
		err = jpeg.Encode(&buf, img, &jpeg.Options{Quality: quality})
	case types.FormatPng:
		encoder := &png.Encoder{CompressionLevel: png.DefaultCompression}
		err = encoder.Encode(&buf, img)
	case types.FormatTiff:
		err = tiff.Encode(&buf, img, &tiff.Options{Compression: tiff.Deflate})
	case types.FormatWebp:
		err = webp.Encode(&buf, img, &webp.Options{Quality: quality})
	default:
		err = errors.Errorf("unsupported image format: %q", format)
	}

	if err != nil {
		return nil, errors.Wrap(err, "failed to encode image")
	}

	return buf.Bytes(), nil
}

// SaveImage encodes data.Pixels in the given format and writes it to data.FilePath, returning the number of bytes
// written. quality is 0-100 and, as in EncodeImage, only affects the lossy formats.
func SaveImage(data *types.ImageData, format types.ImageFormat, quality int) (int, error) {
	if quality < 0 || quality > 100 {
		return 0, errors.Errorf("invalid quality: %d, must be between 0 and 100", quality)
	}

	imageBytes, err := EncodeImage(data.Pixels, format, quality)
	if err != nil {
		return 0, errors.Wrap(err, "failed to encode image")
	}

	err = os.WriteFile(data.FilePath, imageBytes, 0644)
	if err != nil {
		return 0, errors.Wrap(err, "failed to write image file")
	}

	return len(imageBytes), nil
}
