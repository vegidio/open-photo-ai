package detection

import "sort"

// nmsIoUThreshold is the intersection-over-union cutoff above which the lower-scored of two overlapping detections is
// suppressed during non-maximum suppression.
const nmsIoUThreshold = 0.4

// PostProcessDetections processes the model outputs to extract face detections
func PostProcessDetections(
	loc, conf, landmarksRaw []float32,
	origWidth, origHeight, threshold float32,
) []Face {
	// Anchors are memoized for TargetSize and shared (read-only) across calls.
	priors := anchors()

	// Filter by confidence first, decoding boxes/landmarks only for the surviving anchors, then scale to target size.
	filteredBoxes, filteredLandmarks, filteredScores := filterAndScaleDetections(
		loc, landmarksRaw, priors, conf, threshold, float32(TargetSize))

	// Early exit if no detections
	if len(filteredBoxes) == 0 {
		return []Face{}
	}

	// Apply NMS
	keep := nms(filteredBoxes, filteredScores, nmsIoUThreshold)

	// Scale back to the original image size using the same integer dimensions the image was actually resized to
	// (see PreprocessImage/calculateResizeDimensions) so the scale factor matches the preprocessing exactly.
	resizedWidth, resizedHeight := calculateResizeDimensions(origWidth, origHeight, TargetSize)
	scaleW := origWidth / float32(resizedWidth)
	scaleH := origHeight / float32(resizedHeight)

	// Convert to Face structs with original dimensions
	return scaleDetectionsToOriginal(filteredBoxes, filteredLandmarks, filteredScores, keep, scaleW, scaleH)
}

// filterAndScaleDetections selects anchors whose confidence exceeds threshold, decodes only those anchors' boxes and
// landmarks, and scales them to target size. Decoding is deferred to the survivors so the expensive per-anchor decode
// (notably math.Exp) is not paid for the vast majority of anchors that are discarded.
func filterAndScaleDetections(
	loc, landmarksRaw []float32,
	priors []Prior,
	conf []float32,
	threshold, targetSize float32,
) ([]RectF, [][numLandmarks]PointF, []float32) {
	numAnchors := len(conf) / 2

	// First pass: count detections above the threshold
	numFiltered := 0
	for i := 0; i < numAnchors; i++ {
		if conf[i*2+1] > threshold {
			numFiltered++
		}
	}

	// Early exit if no detections
	if numFiltered == 0 {
		return []RectF{}, [][numLandmarks]PointF{}, []float32{}
	}

	// Pre-allocate slices with the exact capacity needed
	filteredBoxes := make([]RectF, 0, numFiltered)
	filteredLandmarks := make([][numLandmarks]PointF, 0, numFiltered)
	filteredScores := make([]float32, 0, numFiltered)

	// Second pass: decode and scale only the anchors that passed the threshold
	for i := 0; i < numAnchors; i++ {
		score := conf[i*2+1]
		if score <= threshold {
			continue
		}

		// Decode and scale the box to target size
		box := decodeBox(loc, priors[i], i)
		box.Min.X *= targetSize
		box.Min.Y *= targetSize
		box.Max.X *= targetSize
		box.Max.Y *= targetSize

		// Decode and scale the landmarks to target size
		lm := decodeLandmark(landmarksRaw, priors[i], i)
		for j := 0; j < numLandmarks; j++ {
			lm[j].X *= targetSize
			lm[j].Y *= targetSize
		}

		filteredBoxes = append(filteredBoxes, box)
		filteredLandmarks = append(filteredLandmarks, lm)
		filteredScores = append(filteredScores, score)
	}

	return filteredBoxes, filteredLandmarks, filteredScores
}

// nms applies non-maximum suppression
func nms(boxes []RectF, scores []float32, threshold float32) []int {
	type scoreIndex struct {
		score float32
		index int
	}

	n := len(scores)

	// Create and sort by score descending
	scoreIndexes := make([]scoreIndex, n)
	for i, s := range scores {
		scoreIndexes[i] = scoreIndex{s, i}
	}

	sort.Slice(scoreIndexes, func(i, j int) bool {
		return scoreIndexes[i].score > scoreIndexes[j].score
	})

	keep := make([]int, 0, n/2)
	suppressed := make([]bool, n)

	// Pre-compute all areas at once. The +1 on each dimension is the Pascal-VOC pixel convention carried over from the
	// reference RetinaFace NMS; it is applied consistently here and to the intersection below, so the IoU ratio is
	// unbiased.
	areas := make([]float32, n)
	for i, box := range boxes {
		areas[i] = (box.Max.X - box.Min.X + 1) * (box.Max.Y - box.Min.Y + 1)
	}

	for pos := 0; pos < len(scoreIndexes); pos++ {
		i := scoreIndexes[pos].index
		if suppressed[i] {
			continue
		}

		keep = append(keep, i)

		box1 := boxes[i]
		area1 := areas[i]
		x1Min := box1.Min.X
		y1Min := box1.Min.Y
		x1Max := box1.Max.X
		y1Max := box1.Max.Y

		// Only check boxes ranked lower than this one. A higher-scored box is processed earlier; if it overlapped
		// this one it would already have suppressed it, so this box can never suppress an un-suppressed higher box.
		for k := pos + 1; k < len(scoreIndexes); k++ {
			j := scoreIndexes[k].index
			if suppressed[j] {
				continue
			}

			box2 := boxes[j]

			// Calculate intersection
			xx1 := x1Min
			if box2.Min.X > xx1 {
				xx1 = box2.Min.X
			}

			yy1 := y1Min
			if box2.Min.Y > yy1 {
				yy1 = box2.Min.Y
			}

			xx2 := x1Max
			if box2.Max.X < xx2 {
				xx2 = box2.Max.X
			}

			yy2 := y1Max
			if box2.Max.Y < yy2 {
				yy2 = box2.Max.Y
			}

			// Early exit if no intersection (+1 matches the Pascal-VOC convention used for the areas above)
			w := xx2 - xx1 + 1
			h := yy2 - yy1 + 1
			if w <= 0 || h <= 0 {
				continue
			}

			inter := w * h

			// Calculate IoU
			iou := inter / (area1 + areas[j] - inter)

			if iou > threshold {
				suppressed[j] = true
			}
		}
	}

	return keep
}

// scaleDetectionsToOriginal scales filtered detections back to original image dimensions
func scaleDetectionsToOriginal(
	filteredBoxes []RectF,
	filteredLandmarks [][numLandmarks]PointF,
	filteredScores []float32,
	keep []int,
	scaleW, scaleH float32,
) []Face {
	faces := make([]Face, 0, len(keep))
	for _, idx := range keep {
		box := filteredBoxes[idx]
		lm := filteredLandmarks[idx]
		score := filteredScores[idx]

		// Create a bounding box rectangle with float32 coordinates
		boundingBox := RectF{
			Min: PointF{
				X: box.Min.X * scaleW,
				Y: box.Min.Y * scaleH,
			},
			Max: PointF{
				X: box.Max.X * scaleW,
				Y: box.Max.Y * scaleH,
			},
		}

		// Create landmark points with float32 coordinates
		var landmarkPoints [numLandmarks]PointF
		for j := 0; j < numLandmarks; j++ {
			landmarkPoints[j] = PointF{
				X: lm[j].X * scaleW,
				Y: lm[j].Y * scaleH,
			}
		}

		faces = append(faces, Face{
			BoundingBox: boundingBox,
			Landmarks:   landmarkPoints,
			Confidence:  score,
		})
	}

	return faces
}
