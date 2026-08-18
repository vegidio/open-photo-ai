package internal

const (
	ModelBaseUrl = "https://huggingface.co/vegidio/open-photo-ai/resolve/main/models"

	// ModelsDir holds the downloaded model files and the manifests recording them, under the user's config directory.
	ModelsDir = "models"

	// RuntimeDir holds the ONNX Runtime and the execution provider libraries shipped beside it. They live in their own
	// directory rather than at the root of the config directory so the manifest can own the whole tree: replacing the
	// runtime then removes the previous version's providers instead of leaving them for the loader to find.
	RuntimeDir = "runtime"

	// EngineCacheDir holds what the execution providers compile from a model - a TensorRT engine, a CoreML MLProgram -
	// in a subdirectory per model. Keeping it out of ModelsDir is what allows one model's cache to be invalidated when
	// that model changes: TensorRT names its engines after the graph inside the model rather than the file we
	// downloaded, so in a shared directory there is no way to tell whose cache is whose.
	EngineCacheDir = "engines"
)

// EngineCacheFor is the directory an execution provider caches what it compiled from one model into, relative to the
// config directory and slash-separated.
//
// It is a function so that the installer, which clears the cache when it replaces a model's weights, and the session
// builder, which points the provider at it, cannot disagree about which directory that is - the same reason ModelFiles
// exists for the file names.
func EngineCacheFor(id string) string {
	return EngineCacheDir + "/" + id
}
