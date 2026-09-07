package internal

import (
	"strings"
	"sync/atomic"
)

// appName is the name of the application using Open Photo AI's library.
//
// This name is used to create a dedicated config directory for the application, where the ONNX runtime, model files and
// their dependencies are stored, under the user's configuration path. It is set by the Initialize() function.
//
// Atomic rather than a plain string because Initialize writes it while download and inference goroutines from a
// previous lifecycle may still be reading it - the same reason logger and fallbackHandler are atomic pointers.
var appName atomic.Pointer[string]

func init() {
	SetAppName("open-photo-ai")
}

// SetAppName sets the config-directory name the library works under. Safe for concurrent use.
func SetAppName(name string) {
	appName.Store(&name)
}

// AppName returns the config-directory name the library is working under.
func AppName() string {
	return *appName.Load()
}

type RemoteModelData struct {
	Name string
	Size int64
	Hash string
}

// modelData is the remote model manifest, populated during Initialize. Atomic for the same reason as appName: the
// manifest is replaced by a re-Initialize while model downloads started by the previous one may still be reading it.
var modelData atomic.Pointer[[]RemoteModelData]

// SetModelData replaces the remote model manifest. Safe for concurrent use.
func SetModelData(data []RemoteModelData) {
	modelData.Store(&data)
}

// ModelData returns the remote model manifest. It is nil when no manifest could be loaded, which callers must read as
// "unverified", not "empty".
func ModelData() []RemoteModelData {
	if data := modelData.Load(); data != nil {
		return *data
	}

	return nil
}

// ModelFiles returns the manifest entries that make up the model behind id: the graph, plus any external-data blob
// stored beside it.
//
// Prefix, not equality, is what groups a model split across several files: both `up_osaka_fp16.onnx` and its
// `up_osaka_fp16.onnx.data` weights blob start with `up_osaka_fp16.onnx`. Matching on that whole stem rather than on id
// alone is what keeps a future `up_osaka_fp16_v2` out of `up_osaka_fp16`'s file set. It is the one place the rule is
// written down, so the size estimate and the download can't drift into matching different files for the same id.
func ModelFiles(id string) []RemoteModelData {
	var found []RemoteModelData

	for _, model := range ModelData() {
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

// imageCache is the disk-backed cache of per-operation results. Atomic because Destroy closes it while an in-flight
// Process may still be holding the lifecycle-free path open, and because a re-Initialize replaces it.
var imageCache atomic.Pointer[Cache]

// SwapImageCache installs cache and returns whatever it replaced, so the caller can close the old one. Passing nil
// clears the cache, which is what Destroy does to keep a closed store from being reachable.
func SwapImageCache(cache *Cache) *Cache {
	return imageCache.Swap(cache)
}

// ImageCache returns the current image cache, or nil when the library was never initialized or has been destroyed.
func ImageCache() *Cache {
	return imageCache.Load()
}
