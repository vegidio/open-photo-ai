package utils

import (
	"image"
	"image/color"
	"math/rand"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// noiseImage builds an image the lossy encoders cannot compress away. A flat colour comes out at roughly the same
// size at every quality setting, which would make the assertions below flake rather than fail honestly.
func noiseImage(width, height int) image.Image {
	// Fixed seed: the sizes compared here should not move between runs.
	rng := rand.New(rand.NewSource(1))
	img := image.NewRGBA(image.Rect(0, 0, width, height))

	for y := range height {
		for x := range width {
			img.Set(x, y, color.RGBA{
				R: uint8(rng.Intn(256)),
				G: uint8(rng.Intn(256)),
				B: uint8(rng.Intn(256)),
				A: 255,
			})
		}
	}

	return img
}

// The point of this test is the wiring, not the codecs: it proves the quality argument actually reaches each lossy
// encoder, which it did not for AVIF, HEIC and WebP before the export quality setting existed.
func TestEncodeImageQualityAffectsLossyFormats(t *testing.T) {
	img := noiseImage(128, 128)

	for _, format := range []types.ImageFormat{types.FormatAvif, types.FormatHeic, types.FormatJpeg, types.FormatWebp} {
		t.Run(string(format), func(t *testing.T) {
			low, err := EncodeImage(img, format, 10)
			if err != nil {
				t.Fatalf("encoding at quality 10 failed: %v", err)
			}

			high, err := EncodeImage(img, format, 90)
			if err != nil {
				t.Fatalf("encoding at quality 90 failed: %v", err)
			}

			if len(low) >= len(high) {
				t.Errorf("quality is ignored: %d bytes at quality 10, %d bytes at quality 90", len(low), len(high))
			}
		})
	}
}

func TestEncodeImageQualityIgnoredByLosslessFormats(t *testing.T) {
	img := noiseImage(64, 64)

	for _, format := range []types.ImageFormat{types.FormatBmp, types.FormatGif, types.FormatPng, types.FormatTiff} {
		t.Run(string(format), func(t *testing.T) {
			low, err := EncodeImage(img, format, 10)
			if err != nil {
				t.Fatalf("encoding at quality 10 failed: %v", err)
			}

			high, err := EncodeImage(img, format, 90)
			if err != nil {
				t.Fatalf("encoding at quality 90 failed: %v", err)
			}

			if len(low) != len(high) {
				t.Errorf("quality changed the output: %d bytes at quality 10, %d bytes at quality 90",
					len(low), len(high))
			}
		})
	}
}

// The zero value of ImageFormat is the empty string precisely so that an unset format is an error rather than
// silently encoding as whatever happens to be first in the list.
func TestEncodeImageRejectsTheZeroFormat(t *testing.T) {
	var format types.ImageFormat

	if _, err := EncodeImage(noiseImage(8, 8), format, 90); err == nil {
		t.Fatal("expected the zero format to be rejected")
	}
}

func TestSaveImageRejectsQualityOutOfRange(t *testing.T) {
	data := &types.ImageData{FilePath: t.TempDir() + "/out.jpg", Pixels: noiseImage(8, 8)}

	for _, quality := range []int{-1, 101} {
		if _, err := SaveImage(data, types.FormatJpeg, quality); err == nil {
			t.Errorf("expected quality %d to be rejected", quality)
		}
	}
}
