package utils

import (
	"bytes"
	"image"
	"math"
	"os"
	"testing"
)

// The fixtures are built by testdata/make_dng_fixtures.py; see it for their layout.
const (
	lossyFixture = "testdata/lossy_linear_raw.dng"
	jxlFixture   = "testdata/jxl_linear_raw.dng"
)

// Mirrors sample8 in make_dng_fixtures.py.
func lossyFixtureSample(x, y int) [3]int {
	return [3]int{x * 255 / 63, y * 255 / 47, (x + y) * 255 / 110}
}

// Mirrors sample16 in make_dng_fixtures.py.
func jxlFixtureSample(x, y int) [3]uint16 {
	return [3]uint16{uint16(x*512 + y), uint16(40000 - x*300 - y*7), uint16(y*1000 + x)}
}

// expandedMain expands path and returns the rewritten file with its main image's directory.
func expandedMain(t *testing.T, path string) (*tiffFile, map[uint16]tiffEntry) {
	t.Helper()

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}

	out, err := expandCompressedDNG(data)
	if err != nil {
		t.Fatalf("expand failed: %v", err)
	}

	f, ok := newTIFF(out)
	if !ok {
		t.Fatal("output is not a TIFF")
	}

	ifd0, err := f.readIFD(f.order.Uint32(out[4:]))
	if err != nil {
		t.Fatal(err)
	}
	subs, err := f.uints(ifd0[tagSubIFDs])
	if err != nil {
		t.Fatal(err)
	}
	main, err := f.readIFD(subs[0])
	if err != nil {
		t.Fatal(err)
	}

	if c, _ := f.uint(main, tagCompression, 0); c != tiffCompressionNone {
		t.Fatalf("compression is %d, want %d", c, tiffCompressionNone)
	}

	return f, main
}

// forEachPixel calls fn with every in-image pixel's 16-bit samples from the expanded, tiled main image.
func forEachPixel(t *testing.T, f *tiffFile, main map[uint16]tiffEntry, fn func(x, y int, got [3]uint16)) {
	t.Helper()

	offsets, _ := f.uints(main[tagTileOffsets])
	counts, _ := f.uints(main[tagTileByteCounts])
	w, _ := f.uint(main, tagImageWidth, 0)
	h, _ := f.uint(main, tagImageLength, 0)
	tw, _ := f.uint(main, tagTileWidth, 0)
	width, height, tile := int(w), int(h), int(tw)
	tilesAcross := (width + tile - 1) / tile

	for i, off := range offsets {
		if int(counts[i]) != tile*tile*3*2 {
			t.Fatalf("tile %d is %d bytes, want %d", i, counts[i], tile*tile*3*2)
		}

		for ty := range tile {
			for tx := range tile {
				x, y := (i%tilesAcross)*tile+tx, (i/tilesAcross)*tile+ty
				if x >= width || y >= height {
					continue
				}

				p := off + uint32((ty*tile+tx)*3*2)
				fn(x, y, [3]uint16{f.order.Uint16(f.data[p:]), f.order.Uint16(f.data[p+2:]), f.order.Uint16(f.data[p+4:])})
			}
		}
	}
}

func TestExpandLossyDNGMapsSamplesThroughItsCurves(t *testing.T) {
	f, main := expandedMain(t, lossyFixture)

	// The samples are now 16-bit linear, so the tags describing them must say so.
	for tag, want := range map[uint16]uint32{tagBitsPerSample: 16, tagWhiteLevel: math.MaxUint16} {
		values, err := f.uints(main[tag])
		if err != nil {
			t.Fatal(err)
		}
		for _, v := range values {
			if v != want {
				t.Fatalf("tag %d is %v, want every value %d", tag, values, want)
			}
		}
	}

	// Plane 0 has a linear polynomial, plane 1 a quadratic, plane 2 none (LibRaw's sRGB fallback).
	srgb := srgbToLinearCurve()
	curves := [3]func(v int) float64{
		func(v int) float64 { return float64(v) / 255 * math.MaxUint16 },
		func(v int) float64 { return math.Pow(float64(v)/255, 2) * math.MaxUint16 },
		func(v int) float64 { return float64(srgb[v]) },
	}

	// Lossy JPEG moves an 8-bit sample by a few levels - up to 4 on this fixture, with libjpeg and Go's decoder alike;
	// every curve is monotonic, so the mapped value must fall between the curve at the source sample minus and plus
	// that tolerance.
	const tolerance = 5
	forEachPixel(t, f, main, func(x, y int, got [3]uint16) {
		for c, source := range lossyFixtureSample(x, y) {
			lo := curves[c](max(source-tolerance, 0)) - 1
			hi := curves[c](min(source+tolerance, 255)) + 1
			if v := float64(got[c]); v < lo || v > hi {
				t.Fatalf("pixel (%d,%d) channel %d = %d, want %.0f..%.0f (source %d)", x, y, c, got[c], lo, hi, source)
			}
		}
	})
}

// JPEG XL tiles are 16-bit and linear already, and lossless in the fixture, so every sample must come back exactly.
func TestExpandJXLDNGDecompressesTilesExactly(t *testing.T) {
	f, main := expandedMain(t, jxlFixture)

	forEachPixel(t, f, main, func(x, y int, got [3]uint16) {
		if want := jxlFixtureSample(x, y); got != want {
			t.Fatalf("pixel (%d,%d) = %v, want %v", x, y, got, want)
		}
	})
}

func TestExpandJXLDNGRejectsLowBitDepthStreams(t *testing.T) {
	original := decodeJXL
	t.Cleanup(func() { decodeJXL = original })
	decodeJXL = func([]byte) (image.Image, error) {
		return image.NewNRGBA(image.Rect(0, 0, 32, 32)), nil
	}

	data, err := os.ReadFile(jxlFixture)
	if err != nil {
		t.Fatal(err)
	}

	if _, err := expandCompressedDNG(data); err == nil {
		t.Fatal("expected an 8-bit decode of a 16-bit JPEG XL DNG to be rejected")
	}
}

func TestSRGBToLinearCurveMatchesTheSRGBTransfer(t *testing.T) {
	curve := srgbToLinearCurve()

	for i, v := range curve {
		r := float64(i) / 255
		want := r / 12.92
		if r > 0.04045 {
			want = math.Pow((r+0.055)/1.055, 2.4)
		}

		// LibRaw scales by 0x10000 and finds the breakpoint numerically, so allow a few units at 16 bits.
		if got := float64(v) / 0x10000; math.Abs(got-want) > 4.0/0x10000 && i != 255 {
			t.Fatalf("curve[%d] = %d (%.5f), want %.5f", i, v, got, want)
		}
	}

	if curve[255] != math.MaxUint16 {
		t.Fatalf("curve[255] = %d, want %d", curve[255], math.MaxUint16)
	}
}

func TestExpandCompressedDNGLeavesOtherFilesAlone(t *testing.T) {
	notTIFF := []byte("not a TIFF at all")
	if out, err := expandCompressedDNG(notTIFF); err != nil || !bytes.Equal(out, notTIFF) {
		t.Fatalf("non-TIFF input changed: err=%v", err)
	}

	// The fixture with its compression tag set to uncompressed is an ordinary DNG and must pass through.
	data, err := os.ReadFile(lossyFixture)
	if err != nil {
		t.Fatal(err)
	}
	f, _ := newTIFF(data)
	entries, _, found, err := f.findCompressedMainIFD()
	if err != nil || !found {
		t.Fatalf("fixture main image not found: %v", err)
	}
	plain := bytes.Clone(data)
	f.order.PutUint16(plain[entries[tagCompression].pos:], tiffCompressionNone)

	if out, err := expandCompressedDNG(plain); err != nil || !bytes.Equal(out, plain) {
		t.Fatalf("uncompressed DNG changed: err=%v", err)
	}
}

func TestExpandCompressedDNGRejectsAMismatchedBitDepth(t *testing.T) {
	data, err := os.ReadFile(lossyFixture)
	if err != nil {
		t.Fatal(err)
	}

	// Declare 16-bit samples for lossy JPEG tiles, which only ever hold 8.
	f, _ := newTIFF(data)
	entries, _, _, _ := f.findCompressedMainIFD()
	bad := bytes.Clone(data)
	if err := f.patch(bad, entries[tagBitsPerSample], 16); err != nil {
		t.Fatal(err)
	}

	if _, err := expandCompressedDNG(bad); err == nil {
		t.Fatal("expected a 16-bit lossy JPEG DNG to be rejected")
	}
}

func TestLoadImageDecodesCompressedDNGs(t *testing.T) {
	for _, path := range []string{lossyFixture, jxlFixture} {
		t.Run(path, func(t *testing.T) {
			img, err := LoadImage(path)
			if err != nil {
				t.Fatalf("LoadImage failed: %v", err)
			}

			if b := img.Pixels.Bounds(); b.Dx() != 64 || b.Dy() != 48 {
				t.Fatalf("decoded %dx%d, want 64x48", b.Dx(), b.Dy())
			}

			// The fixtures are gradients, so a decode that came out flat - all black, say - is as wrong as an error.
			if img.Pixels.At(0, 0) == img.Pixels.At(63, 47) {
				t.Fatal("decoded image is flat")
			}
		})
	}
}
