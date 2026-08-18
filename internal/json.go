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

	tmp := filepath.Join(dir, "."+name+".tmp")
	if err = os.WriteFile(tmp, data, 0o644); err != nil {
		return errors.Wrapf(err, "failed to write %s", name)
	}

	if err = os.Rename(tmp, filepath.Join(dir, name)); err != nil {
		os.Remove(tmp)
		return errors.Wrapf(err, "failed to place %s", name)
	}

	return nil
}
