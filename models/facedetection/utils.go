package facedetection

import (
	"image"
	"sort"
)

// filterOverlappingFaces removes duplicate face detections using Non-Maximum Suppression (NMS).
//
// It keeps the face detection with the highest confidence and eliminates overlapping detections that exceed the IoU
// (Intersection over Union) threshold. A lower iouThreshold means less tolerance for overlap between face bounding
// boxes.
func filterOverlappingFaces(faces []Face, iouThreshold float32) []Face {
	if len(faces) == 0 {
		return faces
	}

	// Sort by confidence (descending)
	sort.Slice(faces, func(i, j int) bool {
		return faces[i].Confidence > faces[j].Confidence
	})

	// Preallocate with estimated capacity to reduce allocations
	filtered := make([]Face, 0, len(faces))

	for len(faces) > 0 {
		// Keep the face with the highest confidence
		bestFace := faces[0]
		filtered = append(filtered, bestFace)

		// Filter out overlapping faces
		remaining := make([]Face, 0, len(faces)-1)
		for i := 1; i < len(faces); i++ {
			if calculateIoU(bestFace.Box, faces[i].Box) < iouThreshold {
				remaining = append(remaining, faces[i])
			}
		}
		faces = remaining
	}

	return filtered
}

// calculateIoU computes the Intersection over Union (IoU) metric between two bounding boxes.
//
// IoU measures the overlap between two rectangles as a ratio of their intersection area to their union area. The result
// ranges from 0 (no overlap) to 1 (complete overlap). This metric is commonly used in Non-Maximum Suppression to
// identify duplicate detections.
func calculateIoU(box1, box2 image.Rectangle) float32 {
	// Early exit if either box is empty
	if box1.Empty() || box2.Empty() {
		return 0
	}

	// Calculate intersection
	inter := box1.Intersect(box2)
	if inter.Empty() {
		return 0
	}

	interArea := inter.Dx() * inter.Dy()
	box1Area := box1.Dx() * box1.Dy()
	box2Area := box2.Dx() * box2.Dy()
	unionArea := box1Area + box2Area - interArea

	return float32(interArea) / float32(unionArea)
}
