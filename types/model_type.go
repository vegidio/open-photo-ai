package types

type ModelType string

const (
	ModelTypeFaceDetection   ModelType = "fd"
	ModelTypeDenoise         ModelType = "dn"
	ModelTypeFaceRecovery    ModelType = "fr"
	ModelTypeLightAdjustment ModelType = "la"
	ModelTypeColorBalance    ModelType = "cb"
	ModelTypeUpscale         ModelType = "up"
)
