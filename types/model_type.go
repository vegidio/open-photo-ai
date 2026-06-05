package types

type ModelType string

const (
	ModelTypeDetection       ModelType = "dt"
	ModelTypeDenoise         ModelType = "dn"
	ModelTypeFaceRecovery    ModelType = "fr"
	ModelTypeLightAdjustment ModelType = "la"
	ModelTypeColorBalance    ModelType = "cb"
	ModelTypeUpscale         ModelType = "up"
)
