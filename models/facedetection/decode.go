package facedetection

import "math"

// decodeBoxes decodes bounding boxes from predictions
func decodeBoxes(loc []float32, priors []Prior) []RectF {
	const (
		variance0 = 0.1
		variance1 = 0.2
	)

	numPriors := len(priors)
	boxes := make([]RectF, numPriors)

	for i := 0; i < numPriors; i++ {
		prior := &priors[i]
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
		boxes[i] = RectF{
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

	return boxes
}

// decodeLandmarks decodes landmarks from predictions
func decodeLandmarks(landmarksRaw []float32, priors []Prior) [][5]PointF {
	const variance = 0.1

	numPriors := len(priors)
	landmarks := make([][5]PointF, numPriors)

	for i := 0; i < numPriors; i++ {
		prior := priors[i]
		rawOffset := i * 10

		var0Sx := variance * prior.sx
		var0Sy := variance * prior.sy

		// Decode all 5 landmarks
		for j := 0; j < 5; j++ {
			landmarks[i][j] = PointF{
				X: prior.cx + landmarksRaw[rawOffset+j*2]*var0Sx,
				Y: prior.cy + landmarksRaw[rawOffset+j*2+1]*var0Sy,
			}
		}
	}

	return landmarks
}
