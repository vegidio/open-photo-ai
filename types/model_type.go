package types

type ModelType string

const (
	ModelTypeFaceDetection   ModelType = "fd"
	ModelTypeFaceRecovery    ModelType = "fd"
	ModelTypeLightAdjustment ModelType = "la"
	ModelTypeUpscale         ModelType = "up"
)
