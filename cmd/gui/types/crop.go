package types

// CropInfo is the per-file flip/rotate/crop the user applied in the Crop/Rotate modal. It is applied to the source image
// (flip → rotate → crop) before any enhancement runs. A zero value (Width <= 0 || Height <= 0) means "no crop".
type CropInfo struct {
	Rotation float64
	FlipH    bool
	FlipV    bool
	Top      int
	Left     int
	Width    int
	Height   int
}
