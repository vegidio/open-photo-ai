package types

// ModelType is the two-letter family prefix that opens every operation id - "dn_stockholm_fp32" is a denoise. It is
// what dispatch keys on, so the prefix and the model named after it cannot disagree.
type ModelType string

const (
	ModelTypeDetection       ModelType = "dt"
	ModelTypeDenoise         ModelType = "dn"
	ModelTypeFaceRecovery    ModelType = "fr"
	ModelTypeLightAdjustment ModelType = "la"
	ModelTypeColorBalance    ModelType = "cb"
	ModelTypeColorization    ModelType = "cl"
	ModelTypeUpscale         ModelType = "up"
	ModelTypeSharpen         ModelType = "sh"
)
