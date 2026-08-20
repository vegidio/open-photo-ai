package deps

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/open-photo-ai/internal"
)

// These tests share the package-level internal.AppName and call t.Setenv, so they are deliberately serial: t.Parallel
// is illegal alongside t.Setenv, and the shared name would be a real race under the -race the test task runs with.

// setup redirects os.UserConfigDir at a fresh temp directory - HOME covers darwin, XDG_CONFIG_HOME linux and AppData
// windows - and returns the application's config directory inside it.
func setup(t *testing.T) string {
	t.Helper()

	root := t.TempDir()
	t.Setenv("HOME", root)
	t.Setenv("XDG_CONFIG_HOME", root)
	t.Setenv("AppData", root)

	internal.AppName = "opai-test"

	dir, err := os.UserConfigDir()
	if err != nil {
		t.Fatalf("failed to resolve the user config directory: %v", err)
	}

	return filepath.Join(dir, internal.AppName)
}

// server hands out named payloads and counts the requests, which is how "already installed" is asserted: the check is
// that nothing was fetched at all, not merely that the bytes ended up the same.
type server struct {
	*httptest.Server
	hits atomic.Int32
}

func newServer(t *testing.T, files map[string]string) *server {
	t.Helper()

	s := &server{}
	s.Server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, ok := files[filepath.Base(r.URL.Path)]
		if !ok {
			w.WriteHeader(http.StatusNotFound)
			return
		}

		s.hits.Add(1)
		w.Write([]byte(body))
	}))

	t.Cleanup(s.Close)
	return s
}

func sha256Of(t *testing.T, body string) string {
	t.Helper()

	sum, err := crypto.Sha256String(body)
	if err != nil {
		t.Fatalf("failed to hash the payload: %v", err)
	}

	return sum
}

func readManifestFor(t *testing.T, dir, name string) Manifest {
	t.Helper()

	m, ok := readManifest(dir, name)
	if !ok {
		t.Fatal("expected a manifest to have been written")
	}

	return m
}

func TestInstallRecordsWhatItWrote(t *testing.T) {
	root := setup(t)
	srv := newServer(t, map[string]string{"model.onnx": "weights"})

	dep := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: srv.URL + "/model.onnx", Sha256: sha256Of(t, "weights")}},
	}

	if err := Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	dir := filepath.Join(root, internal.ModelsDir)
	m := readManifestFor(t, dir, ".model.json")

	if len(m.Files) != 1 {
		t.Fatalf("manifest records %d files, want 1", len(m.Files))
	}
	if m.Files[0].Path != "model.onnx" || m.Files[0].Size != int64(len("weights")) {
		t.Errorf("manifest entry = %+v, want model.onnx at %d bytes", m.Files[0], len("weights"))
	}
	if m.Files[0].Sha256 != sha256Of(t, "weights") {
		t.Errorf("manifest hash = %q, want the hash of the payload", m.Files[0].Sha256)
	}

	// A second install must not touch the network. Comparing the bytes wouldn't prove that - only the request count
	// distinguishes "recognised as present" from "downloaded again and happened to match".
	before := srv.hits.Load()
	if err := Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("second Install: %v", err)
	}
	if got := srv.hits.Load(); got != before {
		t.Errorf("second install made %d requests, want 0", got-before)
	}
}

// An archive in a shared directory would install fine and record nothing, because only an exclusive destination can be
// read back to find out what extraction produced. Every start would then find an empty record and download it again.
func TestInstallRejectsAnArchiveInASharedDirectory(t *testing.T) {
	setup(t)

	dep := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: "https://example.test/model.7z"}},
	}

	if err := Install(context.Background(), dep, nil); err == nil {
		t.Error("Install accepted an archive into a shared directory, want an error")
	}
}

func TestInstallRejectsAHashMismatch(t *testing.T) {
	root := setup(t)
	srv := newServer(t, map[string]string{"model.onnx": "tampered"})

	dep := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: srv.URL + "/model.onnx", Sha256: sha256Of(t, "expected")}},
	}

	if err := Install(context.Background(), dep, nil); err == nil {
		t.Fatal("Install succeeded on a hash mismatch, want an error")
	}

	dir := filepath.Join(root, internal.ModelsDir)

	if _, ok := readManifest(dir, ".model.json"); ok {
		t.Error("a failed install must not leave a manifest")
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("failed to read the models directory: %v", err)
	}
	for _, e := range entries {
		t.Errorf("%s survived a failed install", e.Name())
	}
}

// A cancelled context stands in for every interruption - a crash, a kill, a dropped connection. What matters is that
// nothing is left that a later run could mistake for a finished install.
func TestInstallLeavesNothingWhenCancelled(t *testing.T) {
	root := setup(t)
	srv := newServer(t, map[string]string{"model.onnx": "weights"})

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	dep := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: srv.URL + "/model.onnx"}},
	}

	if err := Install(ctx, dep, nil); err == nil {
		t.Fatal("Install succeeded with a cancelled context, want an error")
	}

	if _, ok := readManifest(filepath.Join(root, internal.ModelsDir), ".model.json"); ok {
		t.Error("a cancelled install must not leave a manifest")
	}
}

// Replacing a dependency has to remove what the previous version installed, and nothing else. In a shared directory
// like models/ the "nothing else" half is what stops one model's upgrade from deleting another's files.
func TestInstallReplacesOnlyItsOwnFiles(t *testing.T) {
	root := setup(t)
	dir := filepath.Join(root, internal.ModelsDir)

	srv := newServer(t, map[string]string{"old.onnx": "v1", "new.onnx": "v2"})

	first := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: srv.URL + "/old.onnx", Sha256: sha256Of(t, "v1")}},
	}
	if err := Install(context.Background(), first, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	// A file belonging to someone else: another model, or a stray the manifest never named.
	stranger := filepath.Join(dir, "other_model.onnx")
	if err := os.WriteFile(stranger, []byte("x"), 0o644); err != nil {
		t.Fatalf("failed to write the unrelated file: %v", err)
	}

	second := first
	second.Sources = []Source{{URL: srv.URL + "/new.onnx", Sha256: sha256Of(t, "v2")}}
	if err := Install(context.Background(), second, nil); err != nil {
		t.Fatalf("second Install: %v", err)
	}

	if _, err := os.Stat(filepath.Join(dir, "old.onnx")); !os.IsNotExist(err) {
		t.Error("the previous version's file must be removed")
	}
	if _, err := os.Stat(filepath.Join(dir, "new.onnx")); err != nil {
		t.Errorf("the new file must be installed: %v", err)
	}
	if _, err := os.Stat(stranger); err != nil {
		t.Errorf("a file this dependency never installed must survive: %v", err)
	}
}

// A file that is deleted or truncated behind the app's back must be noticed, because the steady-state check is the only
// thing standing between a half-present install and a crash inside dlopen.
func TestInstallRepairsDamagedFiles(t *testing.T) {
	root := setup(t)
	dir := filepath.Join(root, internal.ModelsDir)
	srv := newServer(t, map[string]string{"model.onnx": "weights"})

	dep := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: srv.URL + "/model.onnx", Sha256: sha256Of(t, "weights")}},
	}

	tests := []struct {
		name   string
		damage func(t *testing.T, path string)
	}{
		{"deleted", func(t *testing.T, path string) {
			if err := os.Remove(path); err != nil {
				t.Fatalf("failed to delete the file: %v", err)
			}
		}},
		{"truncated", func(t *testing.T, path string) {
			if err := os.WriteFile(path, []byte("we"), 0o644); err != nil {
				t.Fatalf("failed to truncate the file: %v", err)
			}
		}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := Install(context.Background(), dep, nil); err != nil {
				t.Fatalf("Install: %v", err)
			}

			tt.damage(t, filepath.Join(dir, "model.onnx"))

			before := srv.hits.Load()
			if err := Install(context.Background(), dep, nil); err != nil {
				t.Fatalf("repair Install: %v", err)
			}
			if srv.hits.Load() == before {
				t.Error("a damaged install must be re-downloaded")
			}

			body, err := os.ReadFile(filepath.Join(dir, "model.onnx"))
			if err != nil || string(body) != "weights" {
				t.Errorf("file = %q (err %v), want the full payload restored", body, err)
			}
		})
	}
}

// An exclusive directory with no manifest was populated by a version that kept no record, so its contents cannot be
// described and must not be merged with a fresh install: on the NVIDIA libraries that would leave the previous
// release's shared objects behind for the loader to find.
func TestInstallEmptiesAnUnmanagedExclusiveDirectory(t *testing.T) {
	root := setup(t)
	dir := filepath.Join(root, "libs", "cudnn")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("failed to create the library directory: %v", err)
	}

	for _, name := range []string{"libcudnn.so.8", ".version"} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to write %s: %v", name, err)
		}
	}

	srv := newServer(t, map[string]string{"cudnn.txt": "new"})
	dep := Dependency{
		Name:        "cudnn",
		Version:     "cudnn/9.23.1",
		Destination: "libs/cudnn",
		Exclusive:   true,
		Sources:     []Source{{URL: srv.URL + "/cudnn.txt", Sha256: sha256Of(t, "new")}},
	}

	if err := Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	if _, err := os.Stat(filepath.Join(dir, "libcudnn.so.8")); !os.IsNotExist(err) {
		t.Error("the unmanaged library must be removed rather than merged with")
	}
	if _, err := os.Stat(filepath.Join(dir, ".version")); !os.IsNotExist(err) {
		t.Error("the stamp the manifest replaces must be removed")
	}
	if _, err := os.Stat(dir); err != nil {
		t.Errorf("the directory itself must survive, for LD_LIBRARY_PATH: %v", err)
	}
}

// An exclusive dependency records the whole tree an archive produced, not just the file that was downloaded. This is
// the case the previous design could not express: an archive of many files verified by hashing one of them.
func TestInstallRecordsEveryExtractedFile(t *testing.T) {
	root := setup(t)
	srv := newServer(t, map[string]string{"onnx.7z": "archive"})

	extracted := map[string]string{
		"onnxruntime.dylib":                 "runtime",
		"onnxruntime_providers_shared.so":   "shared",
		"nested/onnxruntime_providers_x.so": "provider",
	}
	restore := stubUn7zip(t, extracted)
	defer restore()

	dep := Dependency{
		Name:        "onnx-runtime",
		Version:     "runtime/1.26.0",
		Destination: internal.RuntimeDir,
		Exclusive:   true,
		Sources:     []Source{{URL: srv.URL + "/onnx.7z", Sha256: sha256Of(t, "archive")}},
	}

	if err := Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	dir := filepath.Join(root, internal.RuntimeDir)
	m := readManifestFor(t, dir, ManifestName)

	if len(m.Files) != len(extracted) {
		t.Fatalf("manifest records %d files, want %d", len(m.Files), len(extracted))
	}

	for _, f := range m.Files {
		body, ok := extracted[f.Path]
		if !ok {
			t.Errorf("manifest names %q, which the archive never produced", f.Path)
			continue
		}
		// Size only: an extracted file is not re-hashed, because the archive it came out of was already verified
		// in-stream against its pinned hash and nothing reads a per-file hash back.
		if f.Size != int64(len(body)) {
			t.Errorf("entry for %q = %+v, want size %d", f.Path, f, len(body))
		}
		if f.Sha256 != "" {
			t.Errorf("entry for %q recorded a hash %q; extracted files must not be re-read to hash them",
				f.Path, f.Sha256)
		}
	}

	// The archive is bookkeeping, not content, and must not be left behind or recorded.
	if _, err := os.Stat(filepath.Join(dir, "onnx.7z")); !os.IsNotExist(err) {
		t.Error("the archive must be removed after extraction")
	}
}

// Replacing an archive dependency has to remove the files the previous archive produced, which is what stops a
// provider library from an older runtime lingering next to a newer one.
func TestInstallRemovesTheOldArchiveContents(t *testing.T) {
	root := setup(t)
	dir := filepath.Join(root, internal.RuntimeDir)
	srv := newServer(t, map[string]string{"v1.7z": "one", "v2.7z": "two"})

	restore := stubUn7zip(t, map[string]string{
		"onnxruntime.dylib":               "old",
		"onnxruntime_providers_shared.so": "old-shared",
	})
	first := Dependency{
		Name:        "onnx-runtime",
		Version:     "runtime/1.25.0",
		Destination: internal.RuntimeDir,
		Exclusive:   true,
		Sources:     []Source{{URL: srv.URL + "/v1.7z", Sha256: sha256Of(t, "one")}},
	}
	if err := Install(context.Background(), first, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}
	restore()

	restore = stubUn7zip(t, map[string]string{"onnxruntime.dylib": "new"})
	defer restore()

	second := first
	second.Version = "runtime/1.26.0"
	second.Sources = []Source{{URL: srv.URL + "/v2.7z", Sha256: sha256Of(t, "two")}}
	if err := Install(context.Background(), second, nil); err != nil {
		t.Fatalf("second Install: %v", err)
	}

	if _, err := os.Stat(filepath.Join(dir, "onnxruntime_providers_shared.so")); !os.IsNotExist(err) {
		t.Error("a provider from the previous runtime must not survive the upgrade")
	}

	body, err := os.ReadFile(filepath.Join(dir, "onnxruntime.dylib"))
	if err != nil || string(body) != "new" {
		t.Errorf("runtime = %q (err %v), want the new payload", body, err)
	}
}

// A model split into a graph and a weights blob is one dependency of two files. Neither the download nor the record
// used to handle that: only the first file was ever fetched.
func TestInstallHandlesAMultiFileDependency(t *testing.T) {
	root := setup(t)
	srv := newServer(t, map[string]string{"m.onnx": "graph", "m.onnx.data": "weights-blob"})

	var last float64
	monotonic := true
	onProgress := func(_, _ int64, percent float64) {
		if percent < last {
			monotonic = false
		}
		last = percent
	}

	dep := Dependency{
		Name:        "m",
		Destination: internal.ModelsDir,
		Sources: []Source{
			{URL: srv.URL + "/m.onnx", Sha256: sha256Of(t, "graph"), Size: int64(len("graph"))},
			{URL: srv.URL + "/m.onnx.data", Sha256: sha256Of(t, "weights-blob"), Size: int64(len("weights-blob"))},
		},
	}

	if err := Install(context.Background(), dep, onProgress); err != nil {
		t.Fatalf("Install: %v", err)
	}

	dir := filepath.Join(root, internal.ModelsDir)
	for _, name := range []string{"m.onnx", "m.onnx.data"} {
		if _, err := os.Stat(filepath.Join(dir, name)); err != nil {
			t.Errorf("%s was not installed: %v", name, err)
		}
	}

	if m := readManifestFor(t, dir, ".m.json"); len(m.Files) != 2 {
		t.Errorf("manifest records %d files, want 2", len(m.Files))
	}

	// Progress spans the whole dependency rather than restarting per file, which for a 7 MB graph followed by a 6.8 GB
	// blob is the difference between one run to 100% and two.
	if !monotonic {
		t.Error("progress went backwards between sources")
	}
	if last != 1 {
		t.Errorf("final progress = %v, want 1", last)
	}
}

// Sources are downloaded concurrently, past the parallelism limit, so the reporting has to survive several readers at
// once: the callback must never be entered twice over, the percentage must not go backwards, and the manifest must
// still list the files in the order the dependency declared them rather than the order they finished arriving.
func TestInstallDownloadsSourcesConcurrently(t *testing.T) {
	root := setup(t)

	const count = maxParallelDownloads * 2

	payloads := make(map[string]string, count)
	sources := make([]Source, 0, count)
	names := make([]string, 0, count)

	for i := range count {
		name := fmt.Sprintf("part%d.bin", i)
		// Descending sizes, so the declared order is not the order they finish in.
		body := strings.Repeat("x", (count-i)*4096)

		payloads[name] = body
		names = append(names, name)
	}

	srv := newServer(t, payloads)
	for _, name := range names {
		sources = append(sources, Source{
			URL:    srv.URL + "/" + name,
			Sha256: sha256Of(t, payloads[name]),
			Size:   int64(len(payloads[name])),
		})
	}

	var (
		mu        sync.Mutex
		inside    int
		overlap   bool
		last      float64
		monotonic = true
	)

	onProgress := func(_, _ int64, percent float64) {
		mu.Lock()
		inside++
		if inside > 1 {
			overlap = true
		}
		if percent < last {
			monotonic = false
		}
		last = percent
		inside--
		mu.Unlock()
	}

	dep := Dependency{Name: "many", Destination: internal.ModelsDir, Sources: sources}

	if err := Install(context.Background(), dep, onProgress); err != nil {
		t.Fatalf("Install: %v", err)
	}

	if overlap {
		t.Error("two progress callbacks ran at once; callers are written for one at a time")
	}
	if !monotonic {
		t.Error("progress went backwards across concurrent sources")
	}
	if last != 1 {
		t.Errorf("final progress = %v, want 1", last)
	}

	m := readManifestFor(t, filepath.Join(root, internal.ModelsDir), ".many.json")
	if len(m.Files) != count {
		t.Fatalf("manifest records %d files, want %d", len(m.Files), count)
	}
	for i, f := range m.Files {
		if f.Path != names[i] {
			t.Errorf("manifest entry %d = %q, want %q - declared order was not preserved", i, f.Path, names[i])
		}
	}
}

// Replacing a model must drop what the execution providers compiled from the weights being replaced, and only that
// model's: an engine built for one set of weights means nothing for another.
func TestInstallClearsTheDerivedCache(t *testing.T) {
	root := setup(t)
	srv := newServer(t, map[string]string{"a.onnx": "v1", "b.onnx": "v2"})

	seedEngine := func(t *testing.T, id, body string) string {
		t.Helper()

		dir := filepath.Join(root, internal.EngineCacheDir, id)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatalf("failed to create the engine directory: %v", err)
		}

		path := filepath.Join(dir, "kernel.engine")
		if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
			t.Fatalf("failed to write the engine: %v", err)
		}

		return path
	}

	dep := Dependency{
		Name:        "mine",
		Destination: internal.ModelsDir,
		Sources:     []Source{{URL: srv.URL + "/a.onnx", Sha256: sha256Of(t, "v1")}},
		Derived:     []string{internal.EngineCacheDir + "/mine"},
	}
	if err := Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	mine := seedEngine(t, "mine", "compiled")
	theirs := seedEngine(t, "theirs", "compiled")

	replacement := dep
	replacement.Sources = []Source{{URL: srv.URL + "/b.onnx", Sha256: sha256Of(t, "v2")}}
	if err := Install(context.Background(), replacement, nil); err != nil {
		t.Fatalf("second Install: %v", err)
	}

	if _, err := os.Stat(mine); !os.IsNotExist(err) {
		t.Error("the replaced model's engine must be cleared")
	}
	if _, err := os.Stat(theirs); err != nil {
		t.Errorf("another model's engine must be left alone: %v", err)
	}
}

// SkipVerify is the debug override behind opai.SetSkipModelVerification: a model dropped in by hand has no manifest and
// must still be usable.
func TestInstallSkipVerifyAcceptsWhatIsOnDisk(t *testing.T) {
	root := setup(t)
	dir := filepath.Join(root, internal.ModelsDir)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("failed to create the models directory: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, "model.onnx"), []byte("hand-placed"), 0o644); err != nil {
		t.Fatalf("failed to write the model: %v", err)
	}

	srv := newServer(t, map[string]string{"model.onnx": "remote"})
	dep := Dependency{
		Name:        "model",
		Destination: internal.ModelsDir,
		SkipVerify:  true,
		Sources:     []Source{{URL: srv.URL + "/model.onnx", Sha256: sha256Of(t, "something else")}},
	}

	if err := Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("Install: %v", err)
	}

	if srv.hits.Load() != 0 {
		t.Error("a hand-placed model must be used as it is, without downloading")
	}

	body, err := os.ReadFile(filepath.Join(dir, "model.onnx"))
	if err != nil || string(body) != "hand-placed" {
		t.Errorf("model = %q (err %v), want the hand-placed file untouched", body, err)
	}
}
