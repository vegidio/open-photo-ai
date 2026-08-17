package internal

import (
	"strings"
	"sync"
)

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

// ModelData contains metadata about remote models available for download.
//
// This slice holds information about the model name, size, and hash for verification purposes. It is populated during
// initialization and should not be modified directly.
var ModelData []RemoteModelData

// EstimateModelBytes reports the expected size of the files behind an operation ID, taken from the remote manifest, so
// the registry can free memory *before* an expensive session is built rather than after.
//
// The match is by prefix, like GetModelHash, which is what makes it sum a model split across several files: both
// `up_osaka_fp16.onnx` and its `up_osaka_fp16.onnx.data` weights blob start with `up_osaka_fp16`.
//
// It returns 0 when nothing matches - an operation built from several model files (`up_kyoto_8x_fp32` runs the 4x and
// 2x models in sequence, and neither is named after it), or a manifest that failed to load, since LoadModelData is
// best-effort. Callers must read 0 as "unknown", not "free"; the exact size is charged after the session is built.
func EstimateModelBytes(id string) int64 {
	var total int64
	for _, model := range ModelData {
		if strings.HasPrefix(model.Name, id) {
			total += model.Size
		}
	}

	return total
}

// ModelRegistry is a concurrency-safe map of loaded models keyed by operation ID. Callers must use the provided
// methods; the map inside is not exported.
type ModelRegistry struct {
	mu sync.RWMutex
	m  map[string]any
}

func newModelRegistry() *ModelRegistry {
	return &ModelRegistry{m: make(map[string]any)}
}

// Get returns the model stored under key, if any.
func (r *ModelRegistry) Get(key string) (any, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	v, ok := r.m[key]
	return v, ok
}

// Set stores a model under key.
func (r *ModelRegistry) Set(key string, value any) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.m[key] = value
}

// Drain empties the registry and returns the previous contents so the caller can destroy them outside the lock.
func (r *ModelRegistry) Drain() map[string]any {
	r.mu.Lock()
	defer r.mu.Unlock()
	old := r.m
	r.m = make(map[string]any)
	return old
}

// Registry is where all loaded models are stored.
//
// This variable is set via its helper methods from the `GetOrCreateModel` function and should never be mutated
// directly.
var Registry = newModelRegistry()

// InferenceMu protects the models in Registry from being destroyed while they are in use.
//
// Destroying a model releases its underlying ONNX session, and doing that while another goroutine is running inference
// on it is a use-after-free in native code: it terminates the process rather than raising a panic that could be
// recovered from (see https://github.com/vegidio/open-photo-ai/issues/34). Inference holds the read lock, so runs
// still happen concurrently with each other, while CleanRegistry takes the write lock and therefore waits for the work
// in flight to finish instead of pulling sessions out from under it.
//
// Acquire it only at the library's public entry points - Process, Execute and SuggestEnhancements. A Go RWMutex
// deadlocks on a recursive RLock as soon as a writer is queued, so it must never be taken twice on the same call path.
var InferenceMu sync.RWMutex

var ImageCache *Cache
