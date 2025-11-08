package facedetection

import "image"

type Face struct {
	Box        image.Rectangle // The area of the face
	Confidence float32         // The confidence of the detection
	Landmarks  [][2]float32    // The landmarks of the face
}
