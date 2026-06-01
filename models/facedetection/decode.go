package facedetection

import "math"

// decodeBox decodes the bounding box for a single anchor (prior) from the model's location predictions.
//
// loc is the flat per-anchor location output (4 values per anchor); `i` is the anchor index. Decoding is deferred to a
// single anchor (rather than the whole array), so the caller can decode only the anchors that survive the confidence
// threshold, avoiding the expensive math.Exp calls for the ~16,800 anchors that are otherwise discarded.
func decodeBox(loc []float32, prior Prior, i int) RectF {
	const (
		variance0 = 0.1
		variance1 = 0.2
	)

	locOffset := i * 4

	// Cache variance * prior.sx/sy for reuse
	var0Sx := variance0 * prior.sx
	var0Sy := variance0 * prior.sy

	// Decode center coordinates (x, y)
	boxCx := prior.cx + loc[locOffset]*var0Sx
	boxCy := prior.cy + loc[locOffset+1]*var0Sy

	// Decode width and height using exp
	boxW := prior.sx * float32(math.Exp(float64(loc[locOffset+2]*variance1)))
	boxH := prior.sy * float32(math.Exp(float64(loc[locOffset+3]*variance1)))

	// Calculate half-dimensions once
	halfW := boxW * 0.5
	halfH := boxH * 0.5

	// Convert to corner coordinates and store as RectF
	return RectF{
		Min: PointF{
			X: boxCx - halfW,
			Y: boxCy - halfH,
		},
		Max: PointF{
			X: boxCx + halfW,
			Y: boxCy + halfH,
		},
	}
}

// decodeLandmark decodes the 5 facial landmarks for a single anchor (prior) from the model's landmark predictions.
//
// landmarksRaw is the flat per-anchor landmark output (10 values per anchor); `i` is the anchor index. As with
// decodeBox, decoding is per-anchor, so it runs only for anchors that pass the confidence threshold.
func decodeLandmark(landmarksRaw []float32, prior Prior, i int) [5]PointF {
	const variance = 0.1

	rawOffset := i * 10

	var0Sx := variance * prior.sx
	var0Sy := variance * prior.sy

	var landmarks [5]PointF
	for j := 0; j < 5; j++ {
		landmarks[j] = PointF{
			X: prior.cx + landmarksRaw[rawOffset+j*2]*var0Sx,
			Y: prior.cy + landmarksRaw[rawOffset+j*2+1]*var0Sy,
		}
	}

	return landmarks
}
