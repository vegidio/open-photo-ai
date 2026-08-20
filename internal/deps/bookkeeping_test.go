package deps

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
)

// TestPartStateIsNotRecorded guards the suffix match. A sidecar ends in ".json", so a check for
// ".part" alone misses it - and in an exclusive destination recordTree would then write it into the
// manifest as installed content, to be deleted and reinstalled as though it were a library.
func TestPartStateIsNotRecorded(t *testing.T) {
	dir := t.TempDir()

	for _, name := range []string{
		"onnxruntime.dylib",
		".onnx.7z" + partSuffix,
		".onnx.7z" + partStateSuffix,
		"stray" + tmpSuffix,
		"stale" + oldSuffix,
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to seed %s: %v", name, err)
		}
	}

	files, err := recordTree(dir, ManifestName)
	if err != nil {
		t.Fatalf("recordTree: %v", err)
	}

	if len(files) != 1 || files[0].Path != "onnxruntime.dylib" {
		t.Errorf("recorded %v, want only the installed library", files)
	}
}

// TestEmptyDirPreservesPartFiles is what makes resume reachable for the exclusive destinations. This
// runs precisely when an install was interrupted before it could write a manifest, so clearing the
// partial download here would mean the runtime and the NVIDIA libraries could only ever restart.
func TestEmptyDirPreservesPartFiles(t *testing.T) {
	dir := t.TempDir()

	part := filepath.Join(dir, ".cuda_linux_amd64.7z"+partSuffix)
	state := part + stateSuffix
	stale := filepath.Join(dir, "libcudart.so")

	for _, name := range []string{part, state, stale} {
		if err := os.WriteFile(name, []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to seed %s: %v", name, err)
		}
	}

	if err := EmptyDir(dir); err != nil {
		t.Fatalf("EmptyDir: %v", err)
	}

	for _, name := range []string{part, state} {
		if _, err := os.Stat(name); err != nil {
			t.Errorf("%s was swept away; a partial download is not stale content", filepath.Base(name))
		}
	}

	if _, err := os.Stat(stale); !os.IsNotExist(err) {
		t.Error("the previous version's library survived")
	}
}

// TestStalePartsAreSwept covers the other side: a partial download nobody is going to finish should
// not hold onto gigabytes forever, but one belonging to the install now running must not be touched.
func TestStalePartsAreSwept(t *testing.T) {
	dir := t.TempDir()

	mine, mineState := partPaths(dir, "model.onnx")
	fresh, _ := partPaths(dir, "other.onnx")
	old, oldState := partPaths(dir, "abandoned.onnx")
	renamed := filepath.Join(dir, "onnxruntime.dylib"+oldSuffix)

	for _, name := range []string{mine, mineState, fresh, old, oldState, renamed} {
		if err := os.WriteFile(name, []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to seed %s: %v", name, err)
		}
	}

	long := time.Now().Add(-stalePartAge - time.Hour)
	for _, name := range []string{old, oldState} {
		if err := os.Chtimes(name, long, long); err != nil {
			t.Fatalf("failed to age %s: %v", name, err)
		}
	}

	sweepTransient(dir, []Source{{URL: "https://example.test/models/model.onnx"}})

	for _, name := range []string{mine, mineState, fresh} {
		if _, err := os.Stat(name); err != nil {
			t.Errorf("%s was swept; only aged or superseded parts should go", filepath.Base(name))
		}
	}

	for _, name := range []string{old, oldState, renamed} {
		if _, err := os.Stat(name); !os.IsNotExist(err) {
			t.Errorf("%s survived the sweep", filepath.Base(name))
		}
	}
}

// TestInstallReportsProgressDuringExtraction is the bar not sitting at 100% while a couple of
// gigabytes of LZMA2 goes past. What matters is that reports keep arriving after the transfer ends
// and that they cross the boundary between the two phases.
func TestInstallReportsProgressDuringExtraction(t *testing.T) {
	setup(t)

	archive := "runtime.7z"
	body := "a 7z archive's bytes"

	srv := newServer(t, map[string]string{archive: body})
	restore := stubUn7zip(t, map[string]string{
		"onnxruntime.dylib": "the extracted library",
		"provider.dylib":    "another one",
	})
	defer restore()

	var seen []float64
	onProgress := func(_, _ int64, percent float64) { seen = append(seen, percent) }

	dep := Dependency{
		Name:        "onnx-runtime",
		Destination: internal.RuntimeDir,
		Exclusive:   true,
		Sources:     []Source{{URL: srv.URL + "/" + archive, Sha256: sha256Of(t, body), Size: int64(len(body))}},
	}

	if err := Install(t.Context(), dep, onProgress); err != nil {
		t.Fatalf("Install: %v", err)
	}

	if len(seen) < 2 {
		t.Fatalf("got %d progress reports, want the transfer and the expansion to both show", len(seen))
	}

	for i := 1; i < len(seen); i++ {
		if seen[i] < seen[i-1] {
			t.Fatalf("progress went backwards: %v", seen)
		}
	}

	if last := seen[len(seen)-1]; last != 1 {
		t.Errorf("ended at %v, want 1", last)
	}

	// The transfer is worth downloadShare of the bar, so a report at exactly that point is the
	// handover into extraction - the position the old code jumped straight from 100% to done at.
	var crossed bool
	for _, p := range seen {
		if p >= downloadShare && p < 1 {
			crossed = true
			break
		}
	}

	if !crossed {
		t.Errorf("no report landed in the extraction phase; got %v", seen)
	}

}
