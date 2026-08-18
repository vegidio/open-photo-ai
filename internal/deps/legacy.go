package deps

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
)

// legacyRuntimeGlobs match the files older versions extracted straight into the config directory, before the runtime
// moved into RuntimeDir. Only regular files at the top level are ever considered, so the cache/, logs/, models/, libs/
// and engines/ directories are out of reach whatever the pattern says.
var legacyRuntimeGlobs = []string{
	"onnxruntime*", "libonnxruntime*",
	"*.dll", "*.pdb", "*.so", "*.so.*", "*.dylib",
	"LICENSE*", "ThirdPartyNotices*", "VERSION_NUMBER", "GIT_COMMIT_ID", "Privacy.md", "README*",
}

// modelExtensions are the files that belong to models/ rather than to an execution provider's cache. Everything else
// there is a leftover engine, profile or compiled model from the layout that shared the two.
var modelExtensions = []string{".onnx", ".onnx.data", ".onnx_data"}

// PruneLegacyRuntime removes the ONNX Runtime an older version left at the root of the config directory. It reports how
// many files it deleted.
//
// It is safe to call on every start: after the first success nothing matches, and a fresh installation never had those
// files to begin with. Failures are worth logging but not worth failing a launch over - the runtime now in use is the
// one in RuntimeDir, and a stale file at the root is wasted disk rather than a hazard.
func PruneLegacyRuntime() (int, error) {
	root, err := fs.MkUserConfigDir(internal.AppName)
	if err != nil {
		return 0, errors.Wrap(err, "failed to resolve the config directory")
	}

	entries, err := os.ReadDir(root)
	if err != nil {
		return 0, errors.Wrap(err, "failed to read the config directory")
	}

	removed := 0

	for _, entry := range entries {
		if entry.IsDir() || !matchesAny(entry.Name(), legacyRuntimeGlobs) {
			continue
		}

		if err = os.Remove(filepath.Join(root, entry.Name())); err != nil {
			internal.Log().Warn("failed to remove a legacy runtime file", "file", entry.Name(), "err", err)
			continue
		}

		removed++
	}

	if removed > 0 {
		internal.Log().Info("removed the runtime left at the config root by an older version", "files", removed)
	}

	return removed, nil
}

// PruneLegacyEPCache removes the execution provider caches an older version wrote into models/, back when that
// directory doubled as the TensorRT engine cache and the CoreML model cache. It reports how many entries it deleted.
//
// The `.version` stamp that CleanVersionedCache used to write in models/ is both the trigger and the marker: it exists
// only on an installation that predates this change, and deleting it last makes the sweep run exactly once. Install
// cannot do this job itself - models/ is shared by every model, so it only ever touches the paths its own manifest
// names, and these files were named by nobody.
func PruneLegacyEPCache() (int, error) {
	dir, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return 0, errors.Wrap(err, "failed to resolve the models directory")
	}

	stamp := filepath.Join(dir, ".version")
	if !fs.FileExists(stamp) {
		return 0, nil
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return 0, errors.Wrap(err, "failed to read the models directory")
	}

	removed := 0

	for _, entry := range entries {
		name := entry.Name()
		if name == ".version" || strings.HasPrefix(name, ".") || isModelFile(name) {
			continue
		}

		if err = os.RemoveAll(filepath.Join(dir, name)); err != nil {
			return removed, errors.Wrapf(err, "failed to remove %s from the models directory", name)
		}

		removed++
	}

	if err = os.Remove(stamp); err != nil {
		return removed, errors.Wrap(err, "failed to remove the legacy models stamp")
	}

	internal.Log().Info("removed the provider caches an older version kept in the models directory",
		"entries", removed)

	return removed, nil
}

// region - Private functions

func isModelFile(name string) bool {
	lower := strings.ToLower(name)

	for _, ext := range modelExtensions {
		if strings.HasSuffix(lower, ext) {
			return true
		}
	}

	return false
}

func matchesAny(name string, globs []string) bool {
	for _, glob := range globs {
		if ok, err := filepath.Match(glob, name); err == nil && ok {
			return true
		}
	}

	return false
}

// endregion
