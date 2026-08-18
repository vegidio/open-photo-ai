package deps

import (
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

	un7zip = func(_, target string) error {
		for name, body := range files {
			path := filepath.Join(target, filepath.FromSlash(name))

			if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
				return err
			}
			if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
				return err
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
