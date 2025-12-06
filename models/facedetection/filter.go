package facedetection

import "sort"

// PostProcessDetections processes the model outputs to extract face detections
func PostProcessDetections(
	loc, conf, landmarksRaw []float32,
	origWidth, origHeight, threshold float32,
) []Face {
	const targetSize = 640

	// Calculate resized dimensions
	imRatio := origHeight / origWidth
	var resizedWidth, resizedHeight float32

	if imRatio > 1.0 {
		resizedHeight = targetSize
		resizedWidth = resizedHeight / imRatio
	} else {
		resizedWidth = targetSize
		resizedHeight = resizedWidth * imRatio
	}

	// Generate anchors
	priors := generateAnchors(targetSize)

	// Decode boxes and landmarks once
	boxes := decodeBoxes(loc, priors)
	landmarks := decodeLandmarks(landmarksRaw, priors)

	// Filter and scale detections
	targetSizeFloat := float32(targetSize)
	filteredBoxes, filteredLandmarks, filteredScores := filterAndScaleDetections(
		boxes, landmarks, conf, threshold, targetSizeFloat)

	// Early exit if no detections
	if len(filteredBoxes) == 0 {
		return []Face{}
	}

	// Apply NMS
	keep := nms(filteredBoxes, filteredScores, 0.4)

	// Scale back to the original image size
	scaleW := origWidth / resizedWidth
	scaleH := origHeight / resizedHeight

	// Convert to Face structs with original dimensions
	return scaleDetectionsToOriginal(filteredBoxes, filteredLandmarks, filteredScores, keep, scaleW, scaleH)
}

// filterAndScaleDetections filters detections by threshold and scales them to target size
func filterAndScaleDetections(
	boxes []RectF,
	landmarks [][5]PointF,
	conf []float32,
	threshold, targetSize float32,
) ([]RectF, [][5]PointF, []float32) {
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
		return []RectF{}, [][5]PointF{}, []float32{}
	}

	// Pre-allocate slices with the exact capacity needed
	filteredBoxes := make([]RectF, 0, numFiltered)
	filteredLandmarks := make([][5]PointF, 0, numFiltered)
	filteredScores := make([]float32, 0, numFiltered)

	// Scale to target size and filter in a single pass
	for i := 0; i < numAnchors; i++ {
		score := conf[i*2+1]

		if score > threshold {
			// Scale box to target size
			box := boxes[i]
			box.Min.X *= targetSize
			box.Min.Y *= targetSize
			box.Max.X *= targetSize
			box.Max.Y *= targetSize

			// Scale landmarks to target size
			lm := landmarks[i]
			for j := 0; j < 5; j++ {
				lm[j].X *= targetSize
				lm[j].Y *= targetSize
			}

			filteredBoxes = append(filteredBoxes, box)
			filteredLandmarks = append(filteredLandmarks, lm)
			filteredScores = append(filteredScores, score)
		}
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

	// Pre-compute all areas once
	areas := make([]float32, n)
	for i, box := range boxes {
		areas[i] = (box.Max.X - box.Min.X + 1) * (box.Max.Y - box.Min.Y + 1)
	}

	for _, si := range scoreIndexes {
		i := si.index
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

		// Only check the remaining boxes in sorted order
		for k := 0; k < len(scoreIndexes); k++ {
			j := scoreIndexes[k].index
			if i == j || suppressed[j] {
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

			// Early exit if no intersection
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
	filteredLandmarks [][5]PointF,
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
		var landmarkPoints [5]PointF
		for j := 0; j < 5; j++ {
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
