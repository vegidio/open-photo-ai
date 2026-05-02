package types

type ModelType string

const (
	ModelTypeFaceDetection   ModelType = "fd"
	ModelTypeFaceRecovery    ModelType = "fd"
	ModelTypeLightAdjustment ModelType = "la"
	ModelTypeColorBalance    ModelType = "cb"
	ModelTypeUpscale         ModelType = "up"
)
