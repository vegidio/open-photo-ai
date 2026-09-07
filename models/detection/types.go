package detection

// numLandmarks is the number of facial landmark points the RetinaFace model predicts per face
// (left eye, right eye, nose, left mouth corner, right mouth corner).
const numLandmarks = 5

// ArcfaceTemplateSize is the edge length the template coordinates below are expressed in. Face alignment scales the
// template to whatever tile size a variant runs at, so this is what makes that scaling explicit rather than an
// assumption that every variant happens to be 512.
const ArcfaceTemplateSize = 512

// ArcfaceTemplate is the canonical landmark layout at ArcfaceTemplateSize.
//
// An array rather than a slice: as a slice this was exported *and* mutable, so any importer could reorder or rewrite
// the reference points and silently break face alignment for the whole process. An array is copied on assignment, so
// a caller can only corrupt its own copy.
var ArcfaceTemplate = [numLandmarks]PointF{
	{192.98, 239.95}, // Left eye
	{318.90, 240.19}, // Right eye
	{256.63, 314.02}, // Nose
	{201.26, 371.41}, // Left mouth
	{313.09, 371.15}, // Right mouth
}

// PointF represents a point with float32 coordinates
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
	BoundingBox RectF
	Landmarks   [numLandmarks]PointF
	Confidence  float32
}
