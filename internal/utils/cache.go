package utils

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
)

// CleanEPCache wipes the execution provider cache when it predates the running ONNX Runtime, then stamps the version so
// the next start is a no-op. It reports whether anything was removed.
//
// A stamp rather than a manifest, because this is the one directory whose contents nobody downloads: a TensorRT engine
// or a CoreML MLProgram is compiled locally, under names and in numbers that can't be predicted and against no expected
// hash. A version file is the only check available. It is also the durable one - invalidating the cache off the back of
// "the runtime was just replaced" would lose the invalidation entirely if the process died in between, since the next
// start would find a matching runtime, skip the reinstall and never wipe.
//
// The wipe goes through deps.EmptyDir, which is the same "remove the entries, keep the directory" the installer uses
// when it replaces an exclusive dependency - and keeps the LD_LIBRARY_PATH reason for that in one place.
func CleanEPCache(version string) (bool, error) {
	cacheDir, err := fs.MkUserConfigDir(internal.AppName, internal.EngineCacheDir)
	if err != nil {
		return false, errors.Wrapf(err, "failed to resolve the %s directory", internal.EngineCacheDir)
	}

	versionPath := filepath.Join(cacheDir, ".version")

	current, readErr := os.ReadFile(versionPath)
	if readErr == nil && strings.TrimSpace(string(current)) == version {
		return false, nil
	}

	if err = deps.EmptyDir(cacheDir); err != nil {
		return false, err
	}

	if err = os.WriteFile(versionPath, []byte(version), 0o644); err != nil {
		return false, errors.Wrap(err, "failed to write the engine cache version file")
	}

	return true, nil
}
