package detection

import (
	"math"
	"testing"
)

// The variances below are the RetinaFace constants the decode functions hardcode. They are repeated here rather than
// referenced so the test fails if the implementation's values are edited - which is the whole point of pinning them:
// a wrong variance produces boxes that are plausibly sized and consistently wrong.
const (
	testVariance0 = 0.1
	testVariance1 = 0.2
)

// A zero offset must leave the anchor exactly where it is. This is the case that catches a variance applied to the
// wrong term or a sign flip, both of which still produce a well-formed box.
func TestDecodeBoxWithZeroOffsets(t *testing.T) {
	prior := Prior{cx: 0.5, cy: 0.25, sx: 0.1, sy: 0.2}
	loc := []float32{0, 0, 0, 0}

	got := decodeBox(loc, prior, 0)

	assertClose(t, "Min.X", got.Min.X, prior.cx-prior.sx/2)
	assertClose(t, "Min.Y", got.Min.Y, prior.cy-prior.sy/2)
	assertClose(t, "Max.X", got.Max.X, prior.cx+prior.sx/2)
	assertClose(t, "Max.Y", got.Max.Y, prior.cy+prior.sy/2)
}

// The full decode, computed independently from the reference formula rather than from the implementation.
func TestDecodeBoxAppliesVariances(t *testing.T) {
	prior := Prior{cx: 0.4, cy: 0.6, sx: 0.2, sy: 0.3}
	loc := []float32{0.5, -0.25, 0.75, -0.5}

	got := decodeBox(loc, prior, 0)

	wantCx := prior.cx + loc[0]*testVariance0*prior.sx
	wantCy := prior.cy + loc[1]*testVariance0*prior.sy
	wantW := prior.sx * float32(math.Exp(float64(loc[2]*testVariance1)))
	wantH := prior.sy * float32(math.Exp(float64(loc[3]*testVariance1)))

	assertClose(t, "Min.X", got.Min.X, wantCx-wantW/2)
	assertClose(t, "Min.Y", got.Min.Y, wantCy-wantH/2)
	assertClose(t, "Max.X", got.Max.X, wantCx+wantW/2)
	assertClose(t, "Max.Y", got.Max.Y, wantCy+wantH/2)
}

// Decoding is per-anchor precisely so the caller can skip discarded anchors, which means the index arithmetic has to
// land on the right stride. Reading anchor 1 out of a two-anchor buffer is what a stride of 3 or 5 would break.
func TestDecodeBoxReadsTheIndexedAnchor(t *testing.T) {
	prior := Prior{cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1}

	// Anchor 0 is deliberately non-zero, so a decode that ignores the index cannot accidentally pass.
	loc := []float32{9, 9, 9, 9, 0, 0, 0, 0}

	got := decodeBox(loc, prior, 1)

	assertClose(t, "Min.X", got.Min.X, prior.cx-prior.sx/2)
	assertClose(t, "Max.Y", got.Max.Y, prior.cy+prior.sy/2)
}

// Landmarks use the center variance for both axes and no exponential, so a copy-paste of decodeBox's width term would
// show up here as points drifting outward with the offset magnitude.
func TestDecodeLandmarkWithZeroOffsets(t *testing.T) {
	prior := Prior{cx: 0.3, cy: 0.7, sx: 0.15, sy: 0.15}
	raw := make([]float32, 10)

	got := decodeLandmark(raw, prior, 0)

	for j, point := range got {
		assertClose(t, "landmark X", point.X, prior.cx)
		assertClose(t, "landmark Y", point.Y, prior.cy)

		if j == 0 && len(got) != numLandmarks {
			t.Fatalf("decodeLandmark returned %d points, want %d", len(got), numLandmarks)
		}
	}
}

func TestDecodeLandmarkAppliesVariance(t *testing.T) {
	prior := Prior{cx: 0.3, cy: 0.7, sx: 0.15, sy: 0.25}
	raw := []float32{1, -1, 2, -2, 3, -3, 4, -4, 5, -5}

	got := decodeLandmark(raw, prior, 0)

	for j := range numLandmarks {
		wantX := prior.cx + raw[j*2]*testVariance0*prior.sx
		wantY := prior.cy + raw[j*2+1]*testVariance0*prior.sy

		assertClose(t, "landmark X", got[j].X, wantX)
		assertClose(t, "landmark Y", got[j].Y, wantY)
	}
}

// Ten values per anchor, not eight: the landmark stride is the one most easily confused with the box stride.
func TestDecodeLandmarkReadsTheIndexedAnchor(t *testing.T) {
	prior := Prior{cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1}

	raw := make([]float32, 20)
	for i := range 10 {
		raw[i] = 9
	}

	got := decodeLandmark(raw, prior, 1)

	for _, point := range got {
		assertClose(t, "landmark X", point.X, prior.cx)
		assertClose(t, "landmark Y", point.Y, prior.cy)
	}
}

func assertClose(t *testing.T, name string, got, want float32) {
	t.Helper()

	if math.Abs(float64(got-want)) > 1e-6 {
		t.Errorf("%s = %v, want %v", name, got, want)
	}
}
