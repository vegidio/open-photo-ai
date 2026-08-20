package deps

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

// stubUn7zip replaces extraction with a function that writes a known tree, so the manifest around extraction can be
// exercised without a binary fixture per case. The paths are slash-separated and relative to the target directory.
//
// A real archive is still covered end to end by TestUn7zipFixture; the stub is for the cases where what matters is what
// the install did with the result, not that go-sak can read a .7z.
func stubUn7zip(t *testing.T, files map[string]string) func() {
	t.Helper()

	original := un7zip

	un7zip = func(_ context.Context, _, target string, onProgress func(done, total int64)) error {
		total := int64(0)
		for _, body := range files {
			total += int64(len(body))
		}

		var done int64
		if onProgress != nil {
			onProgress(0, total)
		}

		for name, body := range files {
			path := filepath.Join(target, filepath.FromSlash(name))

			if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
				return err
			}
			if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
				return err
			}

			done += int64(len(body))
			if onProgress != nil {
				onProgress(done, total)
			}
		}

		return nil
	}

	restored := false
	return func() {
		if !restored {
			un7zip = original
			restored = true
		}
	}
}
