package deps

import (
	"github.com/vegidio/open-photo-ai/internal"
)

// ModelDependency describes the model behind an operation ID, expanding it to every file the remote manifest groups
// under that name - the graph, plus the external-data blob when the model is split across two files.
//
// With no manifest entry it falls back to the single file the naming convention promises, unverified. That path used to
// be invisible: a failed manifest fetch turned every expected hash into an empty string that was threaded silently
// through the download, so a whole session ran without verification and nothing said so. Here it is one branch, and it
// says so.
func ModelDependency(id string) Dependency {
	files := internal.ModelFiles(id)

	sources := make([]Source, 0, len(files))
	for _, file := range files {
		sources = append(sources, Source{
			URL:    internal.ModelBaseUrl + "/" + file.Name,
			Sha256: file.Hash,
			Size:   file.Size,
		})
	}

	if len(sources) == 0 {
		internal.Log().Warn("model is not in the remote manifest; downloading it unverified", "model_id", id)
		sources = append(sources, Source{URL: internal.ModelBaseUrl + "/" + id + ".onnx"})
	}

	return Dependency{
		Name:        id,
		Destination: internal.ModelsDir,
		Sources:     sources,
		SkipVerify:  internal.SkipModelVerification(),

		// The execution providers compile a TensorRT engine or a CoreML MLProgram from these weights and cache it
		// here. Replacing the weights has to invalidate it: at best the old engine is wasted disk, at worst it is
		// reused against weights it was never built for.
		Derived: []string{internal.EngineCacheFor(id)},
	}
}
