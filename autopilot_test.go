package opai

import (
	"image"
	"image/color"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// The autopilot heuristics decide what the app suggests before the user has touched anything, so a threshold that
// drifts changes the first impression of every photo. The scan and the three predicates are pure, so they are tested
// directly - SuggestEnhancements itself is not, because its first step downloads a face-detection model.

// solidImage builds a width x height image where every pixel is the given colour.
func solidImage(width, height int, c color.NRGBA) *image.NRGBA {
	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		for x := range width {
			img.SetNRGBA(x, y, c)
		}
	}

	return img
}

func TestScanImageCountsEveryPixel(t *testing.T) {
	stats := scanImage(solidImage(4, 5, color.NRGBA{R: 128, G: 128, B: 128, A: 255}))

	if stats.totalPixels != 20 {
		t.Errorf("totalPixels = %v, want 20", stats.totalPixels)
	}
}

// An empty image must not divide by zero anywhere downstream, and must suggest nothing.
func TestScanImageWithNoPixels(t *testing.T) {
	stats := scanImage(image.NewNRGBA(image.Rect(0, 0, 0, 0)))

	if stats.totalPixels != 0 {
		t.Errorf("totalPixels = %v, want 0", stats.totalPixels)
	}

	if shouldLightAdjustment(stats) || shouldColorBalance(stats) {
		t.Error("an empty image suggested an enhancement, want none")
	}
}

// The fast path indexes the pixel buffer directly and the fallback goes through At().RGBA(). The comment claims the
// two produce identical statistics; this is what holds that claim to account.
func TestScanImagePathsAgree(t *testing.T) {
	const width, height = 9, 7

	nrgba := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := range height {
		for x := range width {
			nrgba.SetNRGBA(x, y, color.NRGBA{R: uint8(x * 25), G: uint8(y * 30), B: uint8(x*10 + y*5), A: 255})
		}
	}

	// image.Gray is not one of the buffer-backed types, so it takes the generic branch. Copying through it would
	// change the pixels, so instead wrap the same NRGBA in a type the fast path does not recognise.
	generic := genericImage{nrgba}

	fast := scanImage(nrgba)
	slow := scanImage(generic)

	if fast != slow {
		t.Errorf("scan paths disagree:\n fast = %+v\n slow = %+v", fast, slow)
	}
}

// genericImage hides the concrete type from utils.RgbPixBuffer, forcing the generic At() loop.
type genericImage struct{ inner image.Image }

func (g genericImage) ColorModel() color.Model { return g.inner.ColorModel() }
func (g genericImage) Bounds() image.Rectangle { return g.inner.Bounds() }
func (g genericImage) At(x, y int) color.Color { return g.inner.At(x, y) }

// Light adjustment fires only when the image is both dark on average AND heavily clipped - either alone is a
// legitimate photographic choice, and suggesting a fix for it is what makes autopilot feel wrong.
func TestShouldLightAdjustment(t *testing.T) {
	tests := []struct {
		name  string
		image *image.NRGBA
		want  bool
	}{
		{
			name:  "mid grey is left alone",
			image: solidImage(10, 10, color.NRGBA{R: 128, G: 128, B: 128, A: 255}),
			want:  false,
		},
		{
			name:  "near black is lifted",
			image: solidImage(10, 10, color.NRGBA{R: 5, G: 5, B: 5, A: 255}),
			want:  true,
		},
		{
			name:  "near white is pulled down",
			image: solidImage(10, 10, color.NRGBA{R: 250, G: 250, B: 250, A: 255}),
			want:  true,
		},
		{
			name:  "dark but unclipped is left alone",
			image: solidImage(10, 10, color.NRGBA{R: 40, G: 40, B: 40, A: 255}),
			want:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := shouldLightAdjustment(scanImage(tt.image)); got != tt.want {
				t.Errorf("shouldLightAdjustment = %v, want %v", got, tt.want)
			}
		})
	}
}

// Colour balance needs two of three independent signals to agree. Two of those three are measured only over
// NEAR-NEUTRAL pixels - the ones that ought to be grey - which is what separates a cast from a deliberately colourful
// photograph. That distinction is the whole design of the heuristic, so both sides of it are pinned here.
func TestShouldColorBalance(t *testing.T) {
	tests := []struct {
		name  string
		image *image.NRGBA
		want  bool
	}{
		{
			name:  "neutral grey has no cast",
			image: solidImage(10, 10, color.NRGBA{R: 128, G: 128, B: 128, A: 255}),
			want:  false,
		},
		{
			// Off-grey enough for the neutral-pixel mean to shift, but still under the saturation cutoff, which is
			// exactly what a white-balance error looks like.
			name:  "a warm cast on near-neutral pixels is caught",
			image: solidImage(10, 10, color.NRGBA{R: 150, G: 130, B: 128, A: 255}),
			want:  true,
		},
		{
			name:  "a cool cast on near-neutral pixels is caught",
			image: solidImage(10, 10, color.NRGBA{R: 128, G: 130, B: 150, A: 255}),
			want:  true,
		},
		{
			// A vivid orange is far past the saturation cutoff, so it contributes no neutral pixels and only the
			// white-balance signal fires - one flag, not two. Correcting a deliberately saturated photo is the
			// failure this two-of-three rule exists to prevent, so it must stay false.
			name:  "a saturated colour is not mistaken for a cast",
			image: solidImage(10, 10, color.NRGBA{R: 200, G: 120, B: 60, A: 255}),
			want:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := shouldColorBalance(scanImage(tt.image)); got != tt.want {
				t.Errorf("shouldColorBalance = %v, want %v", got, tt.want)
			}
		})
	}
}

// Upscale is suggested for anything at or below 4 MP. The boundary is exact because it decides whether a 4 MP photo -
// a very common size - gets a suggestion.
func TestShouldUpscale(t *testing.T) {
	const limit = 4 << 20 // 4194304 pixels

	tests := []struct {
		name          string
		width, height int
		want          bool
	}{
		{"a small photo is upscaled", 800, 600, true},
		{"exactly at the limit is upscaled", limit / 2048, 2048, true},
		{"one pixel over the limit is not", limit/2048 + 1, 2048, false},
		{"a large photo is not", 6000, 4000, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			input := &types.ImageData{Pixels: image.NewNRGBA(image.Rect(0, 0, tt.width, tt.height))}

			if got := shouldUpscale(input); got != tt.want {
				t.Errorf("shouldUpscale(%dx%d) = %v, want %v", tt.width, tt.height, got, tt.want)
			}
		})
	}
}
