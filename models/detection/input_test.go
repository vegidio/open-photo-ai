package detection

import (
	"image"
	"image/color"
	"testing"
)

// The letterbox: the long edge goes to targetSize and the short edge keeps the aspect ratio. Both orientations are
// covered because the implementation branches on the ratio, and only one arm is exercised by a landscape photo.
func TestCalculateResizeDimensions(t *testing.T) {
	tests := []struct {
		name                  string
		width, height         float32
		wantWidth, wantHeight int
	}{
		{"landscape halves the height", 1280, 640, 640, 320},
		{"portrait halves the width", 640, 1280, 320, 640},
		{"square fills both edges", 500, 500, 640, 640},
		{"already square at target", 640, 640, 640, 640},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotWidth, gotHeight := calculateResizeDimensions(tt.width, tt.height, TargetSize)

			if gotWidth != tt.wantWidth || gotHeight != tt.wantHeight {
				t.Errorf("calculateResizeDimensions(%v, %v, %d) = (%d, %d), want (%d, %d)",
					tt.width, tt.height, TargetSize, gotWidth, gotHeight, tt.wantWidth, tt.wantHeight)
			}
		})
	}
}

// A zero or negative dimension used to reach the ratio and produce NaN or +Inf, and int(NaN) is implementation-defined
// in Go - so the bad input became a plausible-looking tensor shape rather than an obviously wrong one.
func TestCalculateResizeDimensionsRejectsDegenerateInput(t *testing.T) {
	for _, tt := range []struct {
		name          string
		width, height float32
	}{
		{"zero width", 0, 100},
		{"zero height", 100, 0},
		{"both zero", 0, 0},
		{"negative width", -100, 100},
	} {
		t.Run(tt.name, func(t *testing.T) {
			gotWidth, gotHeight := calculateResizeDimensions(tt.width, tt.height, TargetSize)

			if gotWidth != TargetSize || gotHeight != TargetSize {
				t.Errorf("calculateResizeDimensions(%v, %v, %d) = (%d, %d), want the square target",
					tt.width, tt.height, TargetSize, gotWidth, gotHeight)
			}
		})
	}
}

// The tensor is BGR/CHW with the per-channel means subtracted. Every one of those three facts is invisible in the
// output of a working model and catastrophic if wrong, so each is pinned separately.
func TestCreateInputTensorDataLayout(t *testing.T) {
	const targetSize = 4
	const width, height = 2, 2

	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		for x := range width {
			img.Set(x, y, color.NRGBA{R: 10, G: 20, B: 30, A: 255})
		}
	}

	got := createInputTensorData(img, width, height, targetSize)

	if len(got) != 3*targetSize*targetSize {
		t.Fatalf("tensor has %d elements, want %d", len(got), 3*targetSize*targetSize)
	}

	channelSize := targetSize * targetSize

	// Channel order is B, G, R - not R, G, B.
	assertClose(t, "B at (0,0)", got[0], 30-meanB)
	assertClose(t, "G at (0,0)", got[channelSize], 20-meanG)
	assertClose(t, "R at (0,0)", got[2*channelSize], 10-meanR)
}

// Everything outside the resized image is the negative mean, which is what a zero pixel becomes after subtraction.
// Padding with a literal 0 instead would feed the model a bright border.
func TestCreateInputTensorDataPadsWithNegativeMean(t *testing.T) {
	const targetSize = 4
	const width, height = 2, 2

	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		for x := range width {
			img.Set(x, y, color.NRGBA{R: 255, G: 255, B: 255, A: 255})
		}
	}

	got := createInputTensorData(img, width, height, targetSize)
	channelSize := targetSize * targetSize

	// (2,0) is to the right of the image on the first row, and (0,2) is below it.
	assertClose(t, "B padding right of the image", got[2], -meanB)
	assertClose(t, "G padding below the image", got[channelSize+2*targetSize], -meanG)
	assertClose(t, "R in the far corner", got[2*channelSize+channelSize-1], -meanR)

	// The image region itself is not padding.
	assertClose(t, "B inside the image", got[0], 255-meanB)
}

// The fast path reads *image.NRGBA's pixel buffer directly and the fallback goes through At().RGBA(). They are two
// separate loops writing the same tensor, so they have to agree - the comment claims bit-identity, and this checks it.
func TestCreateInputTensorDataPathsAgree(t *testing.T) {
	const targetSize = 8
	const width, height = 5, 3

	nrgba := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		for x := range width {
			nrgba.Set(x, y, color.NRGBA{R: uint8(x * 40), G: uint8(y * 60), B: uint8(x*20 + y*10), A: 255})
		}
	}

	// image.RGBA is not the fast path's concrete type, so this goes through the generic branch.
	generic := image.NewRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		for x := range width {
			generic.Set(x, y, nrgba.At(x, y))
		}
	}

	fast := createInputTensorData(nrgba, width, height, targetSize)
	slow := createInputTensorData(generic, width, height, targetSize)

	for i := range fast {
		if fast[i] != slow[i] {
			t.Fatalf("tensors diverge at %d: fast path %v, generic path %v", i, fast[i], slow[i])
		}
	}
}

// PreprocessImage returns the ORIGINAL dimensions, not the resized ones - the caller needs them to scale detections
// back, and returning the resized pair instead would shrink every box by the letterbox factor.
func TestPreprocessImageReturnsOriginalDimensions(t *testing.T) {
	img := image.NewNRGBA(image.Rect(0, 0, 1280, 640))

	data, gotWidth, gotHeight := PreprocessImage(img, TargetSize)

	if gotWidth != 1280 || gotHeight != 640 {
		t.Errorf("PreprocessImage returned dimensions (%v, %v), want the original (1280, 640)", gotWidth, gotHeight)
	}

	if len(data) != 3*TargetSize*TargetSize {
		t.Errorf("tensor has %d elements, want %d", len(data), 3*TargetSize*TargetSize)
	}
}
