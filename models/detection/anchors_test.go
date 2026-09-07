package detection

import (
	"math"
	"testing"
)

// The anchor grid is the coordinate system every decoded box and landmark is expressed in, so an off-by-one here does
// not fail loudly - it shifts every detection by a fraction of the image and looks like a model that got worse.

func TestGenerateAnchorsCount(t *testing.T) {
	// Three pyramid levels at strides 8/16/32, two anchor sizes each. For 640 the feature maps are 80, 40 and 20
	// squares, so the total is 2*(80² + 40² + 20²) = 16800 - the figure the decode path's comments quote.
	const imageSize = 640
	want := 2 * (80*80 + 40*40 + 20*20)

	if got := len(generateAnchors(imageSize)); got != want {
		t.Errorf("generateAnchors(%d) produced %d anchors, want %d", imageSize, got, want)
	}
}

// AnchorCount is what sizes the model's output tensors, so it has to agree with the grid the decode path indexes into.
// If these two ever disagree the mismatch surfaces as an out-of-range read on someone's photo, not here.
func TestAnchorCountMatchesGrid(t *testing.T) {
	if got, want := AnchorCount(), len(generateAnchors(TargetSize)); got != want {
		t.Errorf("AnchorCount() = %d, want %d", got, want)
	}
}

// A non-multiple of the stride must round the feature map up rather than truncate, or the last row and column of the
// image get no anchors at all and faces there are never found.
func TestGenerateAnchorsRoundsFeatureMapUp(t *testing.T) {
	const imageSize = 641

	want := 0
	for _, step := range []int{8, 16, 32} {
		side := (imageSize + step - 1) / step
		want += side * side * 2
	}

	if got := len(generateAnchors(imageSize)); got != want {
		t.Errorf("generateAnchors(%d) produced %d anchors, want %d", imageSize, got, want)
	}
}

// Anchors are normalized to 0-1 and their centers sit at the middle of each cell. Both are assumptions decodeBox makes
// when it adds an offset to prior.cx/cy, so they are pinned rather than left implied.
func TestGenerateAnchorsGeometry(t *testing.T) {
	const imageSize = 640
	priors := generateAnchors(imageSize)

	// The first two anchors are the stride-8 cell at (0,0) at its two sizes, 16 and 32 pixels.
	first := priors[0]
	if !closeEnough(first.cx, 0.5*8/640) || !closeEnough(first.cy, 0.5*8/640) {
		t.Errorf("first anchor center = (%v, %v), want the middle of the first stride-8 cell", first.cx, first.cy)
	}

	if !closeEnough(first.sx, 16.0/640) || !closeEnough(first.sy, 16.0/640) {
		t.Errorf("first anchor size = (%v, %v), want 16/640 square", first.sx, first.sy)
	}

	if second := priors[1]; !closeEnough(second.sx, 32.0/640) {
		t.Errorf("second anchor size = %v, want 32/640", second.sx)
	}

	for i, prior := range priors {
		if prior.cx < 0 || prior.cx > 1 || prior.cy < 0 || prior.cy > 1 {
			t.Fatalf("anchor %d has a center outside 0-1: (%v, %v)", i, prior.cx, prior.cy)
		}

		if prior.sx <= 0 || prior.sy <= 0 {
			t.Fatalf("anchor %d has a non-positive size: (%v, %v)", i, prior.sx, prior.sy)
		}
	}
}

// anchors() memoizes, and the doc says the slice is shared and must not be mutated. Handing out a fresh slice per call
// would be a silent per-frame allocation of 16800 structs.
func TestAnchorsAreMemoized(t *testing.T) {
	first, second := anchors(), anchors()

	if len(first) != len(second) {
		t.Fatalf("anchors() returned %d then %d entries", len(first), len(second))
	}

	if &first[0] != &second[0] {
		t.Error("anchors() returned a different backing array on the second call, want the memoized one")
	}
}

func closeEnough(got, want float32) bool {
	return math.Abs(float64(got-want)) < 1e-6
}
