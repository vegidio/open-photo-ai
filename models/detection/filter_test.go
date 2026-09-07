package detection

import (
	"slices"
	"testing"
)

func box(minX, minY, maxX, maxY float32) RectF {
	return RectF{Min: PointF{X: minX, Y: minY}, Max: PointF{X: maxX, Y: maxY}}
}

// Identical boxes are the unambiguous case: whatever the IoU convention, two boxes in the same place overlap
// completely, and only the higher-scored one may survive.
func TestNmsSuppressesDuplicates(t *testing.T) {
	boxes := []RectF{box(0, 0, 10, 10), box(0, 0, 10, 10)}
	scores := []float32{0.9, 0.5}

	keep := nms(boxes, scores, nmsIoUThreshold)

	if len(keep) != 1 {
		t.Fatalf("nms kept %d boxes, want 1", len(keep))
	}

	if keep[0] != 0 {
		t.Errorf("nms kept index %d, want 0 - the higher-scored box must be the survivor", keep[0])
	}
}

// Boxes that do not touch must all survive, however low their scores. This is what the early-exit on a non-positive
// intersection is for, and getting it wrong silently drops every face but one.
func TestNmsKeepsDisjointBoxes(t *testing.T) {
	boxes := []RectF{box(0, 0, 10, 10), box(100, 100, 110, 110), box(200, 0, 210, 10)}
	scores := []float32{0.3, 0.9, 0.6}

	keep := nms(boxes, scores, nmsIoUThreshold)

	if len(keep) != 3 {
		t.Fatalf("nms kept %d boxes, want all 3", len(keep))
	}

	slices.Sort(keep)
	if !slices.Equal(keep, []int{0, 1, 2}) {
		t.Errorf("nms kept %v, want every index", keep)
	}
}

// Survivors come back in descending score order, which is what makes "the first face" the most confident one.
func TestNmsReturnsSurvivorsByScore(t *testing.T) {
	boxes := []RectF{box(0, 0, 10, 10), box(100, 100, 110, 110), box(200, 200, 210, 210)}
	scores := []float32{0.2, 0.8, 0.5}

	keep := nms(boxes, scores, nmsIoUThreshold)

	if !slices.Equal(keep, []int{1, 2, 0}) {
		t.Errorf("nms kept %v, want [1 2 0] - descending by score", keep)
	}
}

// The threshold is a strict greater-than, so an overlap exactly at the cutoff survives. Pinned because flipping it to
// >= changes how many faces a crowded photo reports, with nothing else to signal the change.
func TestNmsThresholdIsExclusive(t *testing.T) {
	// Two 10x10 boxes (areas 11*11=121 under the +1 convention) overlapping in a 6x6 region -> inter 7*7=49,
	// union 121+121-49 = 193, IoU ~= 0.2539.
	boxes := []RectF{box(0, 0, 10, 10), box(4, 4, 14, 14)}
	scores := []float32{0.9, 0.8}

	iou := float32(49) / float32(121+121-49)

	// Just below the true IoU: the pair counts as overlapping and the weaker box goes.
	if keep := nms(boxes, scores, iou-0.01); len(keep) != 1 {
		t.Errorf("nms with a threshold below the IoU kept %d boxes, want 1", len(keep))
	}

	// Exactly at it: the comparison is strict, so nothing is suppressed.
	if keep := nms(boxes, scores, iou); len(keep) != 2 {
		t.Errorf("nms with a threshold equal to the IoU kept %d boxes, want 2", len(keep))
	}
}

// Suppression is not transitive: a box suppressed by the strongest detection must not go on to suppress a third box
// that the strongest one does not overlap. The `if suppressed[j] { continue }` guard is what enforces that.
func TestNmsSuppressedBoxDoesNotSuppress(t *testing.T) {
	boxes := []RectF{
		box(0, 0, 10, 10), // strongest
		box(1, 1, 11, 11), // overlaps the strongest, suppressed
		box(9, 9, 19, 19), // overlaps the middle one, but not the strongest
	}
	scores := []float32{0.9, 0.8, 0.7}

	keep := nms(boxes, scores, nmsIoUThreshold)

	slices.Sort(keep)
	if !slices.Equal(keep, []int{0, 2}) {
		t.Errorf("nms kept %v, want [0 2] - a suppressed box must not suppress others", keep)
	}
}

func TestNmsWithNoBoxes(t *testing.T) {
	if keep := nms(nil, nil, nmsIoUThreshold); len(keep) != 0 {
		t.Errorf("nms on an empty input kept %d boxes, want 0", len(keep))
	}
}

// conf is interleaved background/face pairs, so the face score is the odd element. Reading the even one instead would
// invert every decision, which is exactly the kind of thing that "works" until it is measured.
func TestFilterAndScaleDetectionsReadsTheFaceScore(t *testing.T) {
	priors := []Prior{
		{cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1},
		{cx: 0.25, cy: 0.25, sx: 0.1, sy: 0.1},
	}

	// Anchor 0 is background-heavy and must be dropped; anchor 1 is a confident face and must survive.
	conf := []float32{0.95, 0.05, 0.1, 0.9}
	loc := make([]float32, len(priors)*4)
	landmarks := make([]float32, len(priors)*10)

	boxes, lms, scores := filterAndScaleDetections(loc, landmarks, priors, conf, 0.5, 1)

	if len(boxes) != 1 || len(lms) != 1 || len(scores) != 1 {
		t.Fatalf("filterAndScaleDetections returned %d boxes / %d landmark sets / %d scores, want 1 of each",
			len(boxes), len(lms), len(scores))
	}

	if scores[0] != 0.9 {
		t.Errorf("surviving score = %v, want 0.9 - the face score is the odd element of each conf pair", scores[0])
	}

	// It must be anchor 1's box, decoded with zero offsets and scaled by the target size of 1.
	assertClose(t, "Min.X", boxes[0].Min.X, 0.25-0.05)
	assertClose(t, "Max.Y", boxes[0].Max.Y, 0.25+0.05)
}

// The threshold is a strict greater-than in both the counting pass and the decoding pass. If the two ever disagree the
// slices are sized for one count and filled with another, which is a panic on a real image rather than a wrong answer.
func TestFilterAndScaleDetectionsThresholdIsExclusive(t *testing.T) {
	priors := []Prior{{cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1}}
	conf := []float32{0.5, 0.5}
	loc := make([]float32, 4)
	landmarks := make([]float32, 10)

	if boxes, _, _ := filterAndScaleDetections(loc, landmarks, priors, conf, 0.5, 1); len(boxes) != 0 {
		t.Errorf("a score exactly at the threshold produced %d boxes, want 0", len(boxes))
	}

	if boxes, _, _ := filterAndScaleDetections(loc, landmarks, priors, conf, 0.49, 1); len(boxes) != 1 {
		t.Errorf("a score above the threshold produced %d boxes, want 1", len(boxes))
	}
}

// Boxes and landmarks are scaled by the same target size, in the same pass. A scale applied to one and not the other
// puts the alignment landmarks outside the face they belong to.
func TestFilterAndScaleDetectionsScalesToTargetSize(t *testing.T) {
	priors := []Prior{{cx: 0.5, cy: 0.5, sx: 0.2, sy: 0.2}}
	conf := []float32{0, 1}
	loc := make([]float32, 4)
	landmarks := make([]float32, 10)

	const targetSize = 640
	boxes, lms, _ := filterAndScaleDetections(loc, landmarks, priors, conf, 0.5, targetSize)

	assertClose(t, "Min.X", boxes[0].Min.X, (0.5-0.1)*targetSize)
	assertClose(t, "Max.X", boxes[0].Max.X, (0.5+0.1)*targetSize)

	// Zero landmark offsets put every point at the anchor center, scaled.
	for _, point := range lms[0] {
		assertClose(t, "landmark X", point.X, 0.5*targetSize)
		assertClose(t, "landmark Y", point.Y, 0.5*targetSize)
	}
}

func TestFilterAndScaleDetectionsWithNothingAboveThreshold(t *testing.T) {
	priors := []Prior{{cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1}}
	conf := []float32{0.9, 0.1}

	boxes, lms, scores := filterAndScaleDetections(make([]float32, 4), make([]float32, 10), priors, conf, 0.5, 1)

	if len(boxes) != 0 || len(lms) != 0 || len(scores) != 0 {
		t.Errorf("expected three empty slices, got %d / %d / %d", len(boxes), len(lms), len(scores))
	}
}

// scaleDetectionsToOriginal maps back out of the letterboxed 640-square space, and it selects by the keep list rather
// than by position - so it has to index the filtered slices, not walk them.
func TestScaleDetectionsToOriginalUsesTheKeepList(t *testing.T) {
	boxes := []RectF{box(0, 0, 10, 10), box(20, 20, 40, 40)}
	lms := [][numLandmarks]PointF{{}, {}}
	for j := range numLandmarks {
		lms[1][j] = PointF{X: 20, Y: 40}
	}
	scores := []float32{0.4, 0.8}

	faces := scaleDetectionsToOriginal(boxes, lms, scores, []int{1}, 2, 0.5)

	if len(faces) != 1 {
		t.Fatalf("scaleDetectionsToOriginal returned %d faces, want 1", len(faces))
	}

	if faces[0].Confidence != 0.8 {
		t.Errorf("confidence = %v, want 0.8 - the kept index's score", faces[0].Confidence)
	}

	// The two axes scale independently, so a swapped scaleW/scaleH shows up here.
	assertClose(t, "Min.X", faces[0].BoundingBox.Min.X, 40)
	assertClose(t, "Min.Y", faces[0].BoundingBox.Min.Y, 10)
	assertClose(t, "Max.X", faces[0].BoundingBox.Max.X, 80)
	assertClose(t, "Max.Y", faces[0].BoundingBox.Max.Y, 20)

	for _, point := range faces[0].Landmarks {
		assertClose(t, "landmark X", point.X, 40)
		assertClose(t, "landmark Y", point.Y, 20)
	}
}

func TestScaleDetectionsToOriginalWithNothingKept(t *testing.T) {
	faces := scaleDetectionsToOriginal([]RectF{box(0, 0, 1, 1)}, [][numLandmarks]PointF{{}}, []float32{1}, nil, 1, 1)

	if len(faces) != 0 {
		t.Errorf("scaleDetectionsToOriginal returned %d faces for an empty keep list, want 0", len(faces))
	}
}
