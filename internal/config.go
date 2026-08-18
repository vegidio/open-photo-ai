package internal

import "strings"

// AppName is the name of the application using Open Photo AI's library.
//
// This name is used to create a dedicated config directory for the application, where the ONNX runtime, model files and
// their dependencies are stored, under the user's configuration path. This variable is set by the Initialize() function
// and should never be changed directly.
var AppName = "open-photo-ai"

type RemoteModelData struct {
	Name string
	Size int64
	Hash string
}

// ModelData is the remote model manifest. It is populated during Initialize and should not be modified directly.
var ModelData []RemoteModelData

// ModelFiles returns the manifest entries that make up the model behind id: the graph, plus any external-data blob
// stored beside it.
//
// Prefix, not equality, is what groups a model split across several files: both `up_osaka_fp16.onnx` and its
// `up_osaka_fp16.onnx.data` weights blob start with `up_osaka_fp16.onnx`. Matching on that whole stem rather than on id
// alone is what keeps a future `up_osaka_fp16_v2` out of `up_osaka_fp16`'s file set. It is the one place the rule is
// written down, so the size estimate and the download can't drift into matching different files for the same id.
func ModelFiles(id string) []RemoteModelData {
	var found []RemoteModelData

	for _, model := range ModelData {
		if strings.HasPrefix(model.Name, id+".onnx") {
			found = append(found, model)
		}
	}

	return found
}

// EstimateModelBytes reports the expected size of the files behind an operation ID, taken from the remote manifest, so
// the registry can free memory *before* an expensive session is built rather than after.
//
// It returns 0 when nothing matches - an operation built from several model files (`up_kyoto_8x_fp32` runs the 4x and
// 2x models in sequence, and neither is named after it), or a manifest that failed to load, since LoadModelData is
// best-effort. Callers must read 0 as "unknown", not "free"; the exact size is charged after the session is built.
func EstimateModelBytes(id string) int64 {
	var total int64
	for _, model := range ModelFiles(id) {
		total += model.Size
	}

	return total
}

var ImageCache *Cache
