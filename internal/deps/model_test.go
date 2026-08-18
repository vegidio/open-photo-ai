package deps

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
)

// TestModelDependency covers the translation from a model id to something installable, which is where the split-model
// case lives: a model can be a graph plus a multi-gigabyte weights blob, and only the graph used to be fetched.
func TestModelDependency(t *testing.T) {
	original := internal.ModelData
	t.Cleanup(func() { internal.ModelData = original })

	internal.ModelData = []internal.RemoteModelData{
		{Name: "dn_stockholm_fp32.onnx", Size: 100, Hash: "aaa"},
		{Name: "up_osaka_fp16.onnx", Size: 10, Hash: "bbb"},
		{Name: "up_osaka_fp16.onnx.data", Size: 5000, Hash: "ccc"},
	}

	t.Run("a single-file model", func(t *testing.T) {
		dep := ModelDependency("dn_stockholm_fp32")

		if len(dep.Sources) != 1 {
			t.Fatalf("sources = %d, want 1", len(dep.Sources))
		}
		if dep.Sources[0].Sha256 != "aaa" || dep.Sources[0].Size != 100 {
			t.Errorf("source = %+v, want the manifest's hash and size", dep.Sources[0])
		}
		if dep.Sources[0].URL != internal.ModelBaseUrl+"/dn_stockholm_fp32.onnx" {
			t.Errorf("url = %q, want the model base url", dep.Sources[0].URL)
		}
	})

	t.Run("a split model carries both files", func(t *testing.T) {
		dep := ModelDependency("up_osaka_fp16")

		if len(dep.Sources) != 2 {
			t.Fatalf("sources = %d, want 2 - the graph and its weights blob", len(dep.Sources))
		}
		if dep.Sources[1].Sha256 != "ccc" || dep.Sources[1].Size != 5000 {
			t.Errorf("weights source = %+v, want the blob's hash and size", dep.Sources[1])
		}
	})

	// Every model must invalidate what the providers compiled from it, and only its own.
	t.Run("the derived cache is scoped to the model", func(t *testing.T) {
		dep := ModelDependency("dn_stockholm_fp32")

		want := internal.EngineCacheDir + "/dn_stockholm_fp32"
		if len(dep.Derived) != 1 || dep.Derived[0] != want {
			t.Errorf("derived = %v, want [%s]", dep.Derived, want)
		}
	})

	// The manifest is named after the model because models/ is shared: a single record per directory would make two
	// concurrent model installs a read-modify-write race over one file.
	t.Run("the manifest is named after the model", func(t *testing.T) {
		if got := ModelDependency("dn_stockholm_fp32").Manifest; got != ".dn_stockholm_fp32.json" {
			t.Errorf("manifest = %q, want a name scoped to the model", got)
		}
	})

	// With no manifest entry the model still has to be installable, but unverified - and that has to be visible in the
	// dependency rather than hidden in an empty hash threaded through the download.
	t.Run("an unknown model falls back to one unverified source", func(t *testing.T) {
		dep := ModelDependency("dn_nowhere_fp32")

		if len(dep.Sources) != 1 {
			t.Fatalf("sources = %d, want 1", len(dep.Sources))
		}
		if dep.Sources[0].Sha256 != "" {
			t.Errorf("hash = %q, want empty; there is nothing to verify against", dep.Sources[0].Sha256)
		}
		if dep.Sources[0].URL != internal.ModelBaseUrl+"/dn_nowhere_fp32.onnx" {
			t.Errorf("url = %q, want the name the convention promises", dep.Sources[0].URL)
		}
	})
}
