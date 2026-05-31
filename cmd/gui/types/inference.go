package types

import "github.com/vegidio/open-photo-ai/models/facedetection"

// InferenceParams carries the extra, per-operation inputs the frontend supplies to ProcessImage/ExportImage alongside
// the operation IDs. It is an extensible bag: new per-operation inputs (e.g. masks, palettes) are added as fields here
// without changing the service method signatures. A struct (rather than a map[string]any) keeps Wails' typed
// marshaling intact across the JS↔Go boundary.
type InferenceParams struct {
	// Faces are the pre-detected faces forwarded to the face-recovery operations (athens/santorini).
	Faces []facedetection.Face
}
