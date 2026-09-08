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

	// TimingCacheDir holds the one TensorRT timing cache every model builds against - the measured latency of each
	// kernel the builder has already tried, as opposed to EngineCacheDir's compiled result.
	//
	// It is shared rather than per-model because that is the only placement that is worth anything. Tactic timings
	// carry between graphs: measured on an RTX 5090 (driver 610.88, ONNX Runtime 1.26, TensorRT 10), building each
	// model cold against a cache the others had seeded rather than against an empty one is worth -59.6% on saitama,
	// -21.2% on kyoto, and -3.5% and -3.0% on tokyo and stockholm, whose layer signatures the others do not share.
	// A per-model timing cache would additionally be pointless here, since it would live in that model's own
	// EngineCacheDir subdirectory and be cleared by the same two paths that clear the engine it was accelerating.
	//
	// Confirmed through perftest on the same machine rather than only in a harness: saitama's cold start is 11.602s
	// with nothing seeded and 4.246s once another model has built, and kyoto's is 18.449s against 14.635s. The
	// steady-state medians are unchanged either way - 169.3ms against 174.1ms and 418.0ms against 412.4ms - which is
	// the part that has to hold, since a timing cache changes which tactics the builder considers already measured
	// and could in principle have it settle for a worse engine.
	//
	// It sits INSIDE EngineCacheDir, which is what gives it the right lifetime at both ends. A model's weights being
	// replaced clears only that model's subdirectory, so the shared cache survives it - and a runtime bump empties
	// EngineCacheDir wholesale, which takes this with it. That second half is the important one: a timing cache
	// records tactics for one TensorRT version, and the runtime is where TensorRT comes from.
	TimingCacheDir = EngineCacheDir + "/.timing"
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
