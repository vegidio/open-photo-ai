package utils

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
)

// CleanVersionedCache wipes everything inside dir - a subdirectory of the user's config directory - when its `.version`
// file doesn't match version, then stamps version so the next start is a no-op. It reports whether anything was
// removed.
//
// Only the entries are removed, never dir itself: on Linux the library directories are put on LD_LIBRARY_PATH before
// the process re-execs (see setLibPathAndRestart in cmd/gui) and the loader permanently skips a search path it finds
// missing.
func CleanVersionedCache(dir, version string) (bool, error) {
	cacheDir, err := fs.MkUserConfigDir(internal.AppName, dir)
	if err != nil {
		return false, errors.Wrapf(err, "failed to resolve %s directory", dir)
	}

	versionPath := filepath.Join(cacheDir, ".version")

	current, readErr := os.ReadFile(versionPath)
	if readErr == nil && strings.TrimSpace(string(current)) == version {
		return false, nil
	}

	entries, err := os.ReadDir(cacheDir)
	if err != nil {
		return false, errors.Wrapf(err, "failed to read %s directory", dir)
	}
	for _, e := range entries {
		if err = os.RemoveAll(filepath.Join(cacheDir, e.Name())); err != nil {
			return false, errors.Wrapf(err, "failed to remove %s from the %s cache", e.Name(), dir)
		}
	}

	if err = os.WriteFile(versionPath, []byte(version), 0o644); err != nil {
		return false, errors.Wrapf(err, "failed to write the %s cache version file", dir)
	}

	return true, nil
}
