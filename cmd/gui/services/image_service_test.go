package services

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func TestGetOutputPathOverwriteReturnsPathUnclaimed(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "photo.jpg")

	got, release := getOutputPath(path, true)
	defer release()

	if got != path {
		t.Fatalf("got %q, want %q", got, path)
	}

	// Overwrite mode must not create a placeholder — the caller writes straight over whatever is there.
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("expected no file to be created, stat err = %v", err)
	}
}

func TestGetOutputPathDedupSuffix(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "photo.jpg")

	if err := os.WriteFile(path, []byte("existing"), 0644); err != nil {
		t.Fatal(err)
	}

	got, release := getOutputPath(path, false)
	defer release()

	want := filepath.Join(dir, "photo_1.jpg")
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestGetOutputPathReleaseFreesTheName(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "photo.jpg")

	got, release := getOutputPath(path, false)
	if got != path {
		t.Fatalf("got %q, want %q", got, path)
	}

	release()

	// After a failed export the placeholder must be gone, so the next attempt reuses the original name rather than
	// silently drifting to photo_1.jpg.
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("expected release to remove the placeholder, stat err = %v", err)
	}
}

// TestGetOutputPathConcurrentExportsAreUnique is the point of claiming names with O_EXCL: a stat-then-write check lets
// two exports finishing at the same time both pick the same suffix, and one silently overwrites the other.
func TestGetOutputPathConcurrentExportsAreUnique(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "photo.jpg")

	const exports = 16

	var (
		wg      sync.WaitGroup
		mu      sync.Mutex
		results = make(map[string]int, exports)
	)

	start := make(chan struct{})
	for range exports {
		wg.Add(1)
		go func() {
			defer wg.Done()
			<-start

			got, _ := getOutputPath(path, false)

			mu.Lock()
			results[got]++
			mu.Unlock()
		}()
	}

	close(start)
	wg.Wait()

	if len(results) != exports {
		for p, n := range results {
			if n > 1 {
				t.Errorf("%q handed out %d times", p, n)
			}
		}
		t.Fatalf("got %d distinct paths for %d concurrent exports", len(results), exports)
	}

	for i := range exports {
		want := path
		if i > 0 {
			want = filepath.Join(dir, fmt.Sprintf("photo_%d.jpg", i))
		}
		if _, ok := results[want]; !ok {
			t.Errorf("expected %q to be among the assigned paths", want)
		}
	}
}
