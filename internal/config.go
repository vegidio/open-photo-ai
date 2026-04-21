package internal

import "sync"

// AppName is the name of the application using Open Photo AI's library.
//
// This name is used to create a dedicated config directory for the application, where the ONNX runtime, model files and
// their dependencies are stored, under the user's configuration path. This variable is set by the Initialize() function
// and should never be changed directly.
var AppName = "open-photo-ai"

type RemoteModelData struct {
	Name string
	Size int
	Hash string
}

// ModelData contains metadata about remote models available for download.
//
// This slice holds information about the model name, size, and hash for verification purposes. It is populated during
// initialization and should not be modified directly.
var ModelData []RemoteModelData

// ModelRegistry is a concurrency-safe map of loaded models keyed by operation ID. Callers must use the provided
// methods; the map inside is not exported.
type ModelRegistry struct {
	mu sync.RWMutex
	m  map[string]interface{}
}

func newModelRegistry() *ModelRegistry {
	return &ModelRegistry{m: make(map[string]interface{})}
}

// Get returns the model stored under key, if any.
func (r *ModelRegistry) Get(key string) (interface{}, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	v, ok := r.m[key]
	return v, ok
}

// Set stores a model under key.
func (r *ModelRegistry) Set(key string, value interface{}) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.m[key] = value
}

// Drain empties the registry and returns the previous contents so the caller can destroy them outside the lock.
func (r *ModelRegistry) Drain() map[string]interface{} {
	r.mu.Lock()
	defer r.mu.Unlock()
	old := r.m
	r.m = make(map[string]interface{})
	return old
}

// Registry is where all loaded models are stored.
//
// This variable is set via its helper methods from the `selectModel` function and should never be mutated directly.
var Registry = newModelRegistry()

var ImageCache *Cache
