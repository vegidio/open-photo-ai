package internal

import (
	"encoding/json"
	"os"
	"path/filepath"

	"github.com/cockroachdb/errors"
)

// WriteJSONAtomic encodes v and places it at dir/name through a temporary file, so the document is
// only ever observed complete: an interrupted write leaves the previous one, or none at all, and
// both are answers a reader can act on. A truncated file is not - it would parse as a shorter
// record, which for an install manifest means "these files are mine" about a list missing entries.
//
// The temporary is created beside the target rather than in the system temp directory, because a
// rename is only atomic within one filesystem.
func WriteJSONAtomic(dir, name string, v any) error {
	data, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return errors.Wrapf(err, "failed to encode %s", name)
	}

	// A unique temporary per writer, rather than a fixed ".<name>.tmp". Two processes sharing one config directory -
	// a second instance of the app, or a retried Initialize - would otherwise write into the same file and rename the
	// interleaved result into place, which is precisely the truncated document this function exists to prevent.
	f, err := os.CreateTemp(dir, "."+name+".*")
	if err != nil {
		return errors.Wrapf(err, "failed to create a temporary file for %s", name)
	}

	tmp := f.Name()

	if _, err = f.Write(data); err != nil {
		f.Close()
		os.Remove(tmp)

		return errors.Wrapf(err, "failed to write %s", name)
	}

	if err = f.Close(); err != nil {
		os.Remove(tmp)
		return errors.Wrapf(err, "failed to write %s", name)
	}

	// CreateTemp makes the file 0600; the manifests it writes are meant to be world-readable like the rest of the
	// config directory.
	if err = os.Chmod(tmp, 0o644); err != nil {
		os.Remove(tmp)
		return errors.Wrapf(err, "failed to set permissions on %s", name)
	}

	if err = os.Rename(tmp, filepath.Join(dir, name)); err != nil {
		os.Remove(tmp)
		return errors.Wrapf(err, "failed to place %s", name)
	}

	return nil
}
