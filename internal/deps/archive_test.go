package deps

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/open-photo-ai/internal"
)

// TestInstallExtractsARealArchive runs one install against an actual .7z, which the stub used elsewhere cannot do: the
// stub proves what Install does with an extraction's result, not that go-sak can read the format or that a nested
// directory survives it.
//
// The fixture is testdata/tree.7z - three small files, one of them nested - and mirrors the shape of the real runtime
// archives, where a library ships alongside its execution providers.
func TestInstallExtractsARealArchive(t *testing.T) {
	root := setup(t)

	archive, err := os.ReadFile(filepath.Join("testdata", "tree.7z"))
	if err != nil {
		t.Fatalf("failed to read the fixture: %v", err)
	}

	sum, err := crypto.Sha256Bytes(archive)
	if err != nil {
		t.Fatalf("failed to hash the fixture: %v", err)
	}

	srv := newServer(t, map[string]string{"tree.7z": string(archive)})

	dep := Dependency{
		Name:        "onnx-runtime",
		Version:     "runtime/1.26.0",
		Destination: internal.RuntimeDir,
		Exclusive:   true,
		Sources:     []Source{{URL: srv.URL + "/tree.7z", Sha256: sum}},
	}

	if err = Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	dir := filepath.Join(root, internal.RuntimeDir)

	want := map[string]string{
		"onnxruntime.dylib":                    "runtime",
		"onnxruntime_providers_shared.so":      "shared",
		"nested/onnxruntime_providers_cuda.so": "provider",
	}

	m := readManifestFor(t, dir, ManifestName)
	if len(m.Files) != len(want) {
		t.Fatalf("manifest records %d files, want %d: %+v", len(m.Files), len(want), m.Files)
	}

	for _, f := range m.Files {
		body, ok := want[f.Path]
		if !ok {
			t.Errorf("manifest names %q, which the archive does not contain", f.Path)
			continue
		}

		onDisk, readErr := os.ReadFile(filepath.Join(dir, filepath.FromSlash(f.Path)))
		if readErr != nil || string(onDisk) != body {
			t.Errorf("%s = %q (err %v), want %q", f.Path, onDisk, readErr, body)
		}
	}

	if _, err = os.Stat(filepath.Join(dir, "tree.7z")); !os.IsNotExist(err) {
		t.Error("the archive must be removed once extracted")
	}
}

// A mismatched archive must be rejected before it is expanded, which is the whole point of hashing the .7z rather than
// something inside it: nothing from a bad download ever reaches the directory.
func TestInstallDoesNotExtractAMismatchedArchive(t *testing.T) {
	root := setup(t)

	archive, err := os.ReadFile(filepath.Join("testdata", "tree.7z"))
	if err != nil {
		t.Fatalf("failed to read the fixture: %v", err)
	}

	srv := newServer(t, map[string]string{"tree.7z": string(archive)})

	dep := Dependency{
		Name:        "onnx-runtime",
		Version:     "runtime/1.26.0",
		Destination: internal.RuntimeDir,
		Exclusive:   true,
		Sources: []Source{{
			URL:    srv.URL + "/tree.7z",
			Sha256: "0000000000000000000000000000000000000000000000000000000000000000",
		}},
	}

	if err = Install(context.Background(), dep, nil); err == nil {
		t.Fatal("Install succeeded on a mismatched archive, want an error")
	}

	entries, err := os.ReadDir(filepath.Join(root, internal.RuntimeDir))
	if err != nil {
		t.Fatalf("failed to read the runtime directory: %v", err)
	}
	for _, e := range entries {
		t.Errorf("%s was written despite the archive failing verification", e.Name())
	}
}
