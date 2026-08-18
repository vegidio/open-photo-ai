package deps

import (
	"os"
	"path/filepath"
	"testing"
)

func TestManifestRoundTrip(t *testing.T) {
	dir := t.TempDir()

	want := Manifest{
		Schema:      manifestSchema,
		Name:        "onnx-runtime",
		Version:     "runtime/1.26.0",
		Fingerprint: "abc",
		Files: []File{
			{Path: "onnxruntime.dylib", Size: 7, Sha256: "deadbeef"},
			{Path: "nested/provider.so", Size: 3},
		},
	}

	if err := writeManifest(dir, ManifestName, want); err != nil {
		t.Fatalf("writeManifest: %v", err)
	}

	got, ok := readManifest(dir, ManifestName)
	if !ok {
		t.Fatal("readManifest reported no manifest")
	}
	if got.Fingerprint != want.Fingerprint || len(got.Files) != len(want.Files) {
		t.Errorf("manifest = %+v, want %+v", got, want)
	}
	if got.Files[1].Sha256 != "" {
		t.Errorf("an empty hash must round-trip as empty, got %q", got.Files[1].Sha256)
	}

	// The temporary file the write goes through must not be left behind, or hashTree would record it as content.
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("failed to read the directory: %v", err)
	}
	if len(entries) != 1 {
		t.Errorf("directory holds %d entries, want only the manifest", len(entries))
	}
}

// A manifest that cannot be trusted must read as absent rather than as an error, because every caller would have to
// turn that error back into "reinstall" anyway.
func TestReadManifestTreatsUnusableRecordsAsAbsent(t *testing.T) {
	tests := []struct {
		name string
		body string
	}{
		{"malformed", "{not json"},
		{"another schema", `{"schema":99,"files":[{"path":"a","size":1}]}`},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			if err := os.WriteFile(filepath.Join(dir, ManifestName), []byte(tt.body), 0o644); err != nil {
				t.Fatalf("failed to write the manifest: %v", err)
			}

			if _, ok := readManifest(dir, ManifestName); ok {
				t.Error("readManifest accepted a record it should not trust")
			}
		})
	}

	t.Run("missing", func(t *testing.T) {
		if _, ok := readManifest(t.TempDir(), ManifestName); ok {
			t.Error("readManifest reported a manifest that does not exist")
		}
	})
}

func TestManifestIntact(t *testing.T) {
	setupDir := func(t *testing.T) string {
		t.Helper()

		dir := t.TempDir()
		if err := os.MkdirAll(filepath.Join(dir, "nested"), 0o755); err != nil {
			t.Fatalf("failed to create the nested directory: %v", err)
		}
		if err := os.WriteFile(filepath.Join(dir, "a.so"), []byte("aaa"), 0o644); err != nil {
			t.Fatalf("failed to write a.so: %v", err)
		}
		if err := os.WriteFile(filepath.Join(dir, "nested", "b.so"), []byte("bb"), 0o644); err != nil {
			t.Fatalf("failed to write b.so: %v", err)
		}

		return dir
	}

	m := Manifest{Files: []File{
		{Path: "a.so", Size: 3},
		{Path: "nested/b.so", Size: 2},
	}}

	t.Run("all present", func(t *testing.T) {
		if !m.intact(setupDir(t)) {
			t.Error("intact = false, want true when every file is present at its recorded size")
		}
	})

	t.Run("one missing", func(t *testing.T) {
		dir := setupDir(t)
		os.Remove(filepath.Join(dir, "nested", "b.so"))

		if m.intact(dir) {
			t.Error("intact = true, want false when a file is missing")
		}
	})

	// Size is the only cheap signal that catches a half-written file, which is what an interrupted extraction leaves.
	t.Run("one truncated", func(t *testing.T) {
		dir := setupDir(t)
		if err := os.WriteFile(filepath.Join(dir, "a.so"), []byte("a"), 0o644); err != nil {
			t.Fatalf("failed to truncate a.so: %v", err)
		}

		if m.intact(dir) {
			t.Error("intact = true, want false when a file's size has changed")
		}
	})

	// models/ is shared, so files this manifest never named are none of its business.
	t.Run("an unrecorded sibling is ignored", func(t *testing.T) {
		dir := setupDir(t)
		if err := os.WriteFile(filepath.Join(dir, "someone_else.onnx"), []byte("x"), 0o644); err != nil {
			t.Fatalf("failed to write the sibling: %v", err)
		}

		if !m.intact(dir) {
			t.Error("intact = false, want true; an unrecorded file must not invalidate the record")
		}
	})

	t.Run("an empty record is never intact", func(t *testing.T) {
		if (Manifest{}).intact(t.TempDir()) {
			t.Error("intact = true, want false for a manifest naming no files")
		}
	})
}

func TestManifestRemove(t *testing.T) {
	dir := t.TempDir()

	if err := os.MkdirAll(filepath.Join(dir, "nested", "deep"), 0o755); err != nil {
		t.Fatalf("failed to create the nested directories: %v", err)
	}

	files := map[string]string{
		"a.so":               "x",
		"nested/deep/b.so":   "x",
		"someone_else.onnx":  "x",
		"nested/keeper.data": "x",
	}
	for name, body := range files {
		if err := os.WriteFile(filepath.Join(dir, filepath.FromSlash(name)), []byte(body), 0o644); err != nil {
			t.Fatalf("failed to write %s: %v", name, err)
		}
	}

	m := Manifest{Files: []File{{Path: "a.so"}, {Path: "nested/deep/b.so"}}}
	if err := m.remove(dir); err != nil {
		t.Fatalf("remove: %v", err)
	}

	for _, name := range []string{"a.so", "nested/deep/b.so"} {
		if _, err := os.Stat(filepath.Join(dir, filepath.FromSlash(name))); !os.IsNotExist(err) {
			t.Errorf("%s was recorded and must be removed", name)
		}
	}
	for _, name := range []string{"someone_else.onnx", "nested/keeper.data"} {
		if _, err := os.Stat(filepath.Join(dir, filepath.FromSlash(name))); err != nil {
			t.Errorf("%s was not recorded and must survive: %v", name, err)
		}
	}

	// An emptied subdirectory is pruned, but one still holding something is not.
	if _, err := os.Stat(filepath.Join(dir, "nested", "deep")); !os.IsNotExist(err) {
		t.Error("an emptied subdirectory must be pruned")
	}
	if _, err := os.Stat(filepath.Join(dir, "nested")); err != nil {
		t.Errorf("a subdirectory still holding a file must survive: %v", err)
	}

	// The destination itself always survives: on Linux it is already on LD_LIBRARY_PATH, and the loader permanently
	// skips a search path it finds missing.
	if _, err := os.Stat(dir); err != nil {
		t.Errorf("the destination directory must survive: %v", err)
	}
}

// The fingerprint is what decides a reinstall, so it has to move when either the tag or any source does - a model has
// no tag to bump, and its hash changing upstream is the only signal there is.
func TestFingerprint(t *testing.T) {
	base := Dependency{
		Version: "runtime/1.26.0",
		Sources: []Source{{URL: "https://example.test/a.7z", Sha256: "aaa"}},
	}

	same := base
	same.Name = "a different name is not part of the content"

	if fingerprint(base) != fingerprint(same) {
		t.Error("fingerprint changed on a field that does not affect the installed content")
	}

	tests := []struct {
		name   string
		mutate func(d *Dependency)
	}{
		{"version", func(d *Dependency) { d.Version = "runtime/1.27.0" }},
		{"url", func(d *Dependency) { d.Sources[0].URL = "https://example.test/b.7z" }},
		{"hash", func(d *Dependency) { d.Sources[0].Sha256 = "bbb" }},
		{"an added source", func(d *Dependency) {
			d.Sources = append(d.Sources, Source{URL: "https://example.test/c.data"})
		}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			changed := base
			changed.Sources = append([]Source(nil), base.Sources...)
			tt.mutate(&changed)

			if fingerprint(changed) == fingerprint(base) {
				t.Errorf("fingerprint did not change when the %s did", tt.name)
			}
		})
	}
}
