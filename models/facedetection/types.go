package facedetection

// ArcfaceTemplate is a template landmark at 512x512
var ArcfaceTemplate = []PointF{
	{192.98, 239.95}, // Left eye
	{318.90, 240.19}, // Right eye
	{256.63, 314.02}, // Nose
	{201.26, 371.41}, // Left mouth
	{313.09, 371.15}, // Right mouth
}

// PointF represents a point with float64 coordinates
type PointF struct {
	X float32
	Y float32
}

// RectF represents a rectangle with PointF coordinates
type RectF struct {
	Min PointF
	Max PointF
}

// Face represents a detected face with its properties
type Face struct {
	BoundingBox RectF     // Rectangle for face bounds
	Landmarks   [5]PointF // 5 facial landmark points
	Confidence  float32
}
