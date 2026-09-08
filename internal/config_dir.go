package internal

import (
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
)

// ConfigDir resolves a slash-separated path under the application's config directory, creating it if it isn't there.
//
// Every caller goes through here rather than calling fs.MkUserConfigDir directly, because some of these paths are
// composed from a model id and a model id is not always ours: the GUI parses one out of a string the frontend sent
// (see IdsToOperations), and the precision segment of that id lands verbatim in EngineCacheFor. fs.MkUserConfigDir
// joins the segments with filepath.Join, which resolves "..", and then MkdirAll's the result - after which callers hand
// the directory to deps.EmptyDir, which removes everything in it. A traversal reaching that combination is an arbitrary
// directory wipe, so the segments are checked before anything touches the filesystem.
//
// The check is deliberately a rejection rather than a sanitisation: a caller that produced "engines/../.." has a bug or
// is being fed hostile input, and quietly repairing the path would hide both.
func ConfigDir(rel string) (string, error) {
	parts := strings.Split(rel, "/")

	for _, part := range parts {
		// Backslash matters as much as "..": Split only breaks on "/", so on Windows a segment carrying a backslash
		// still reaches filepath.Join as a separator.
		if part == "" || part == "." || part == ".." || strings.ContainsAny(part, `\/`) {
			return "", errors.Newf("invalid config path %q", rel)
		}
	}

	dir, err := fs.MkUserConfigDir(AppName(), parts...)
	if err != nil {
		return "", errors.Wrapf(err, "failed to resolve the %s directory", rel)
	}

	return dir, nil
}
