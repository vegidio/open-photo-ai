package deps

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
)

// manifestSchema versions the on-disk format. A record written by a different schema is treated as absent, which costs
// a reinstall rather than a misread.
const manifestSchema = 1

// The suffixes marking a file as bookkeeping rather than installed content. They are declared together because they
// are a contract between the code that creates them and the three places that recognise them - isTransient, which keeps
// them out of a manifest and out of EmptyDir's reach, and sweepTransient, which deletes what an earlier install left
// behind.
const (
	partSuffix      = ".part" // a download in progress
	stateSuffix     = ".json" // appended to a part file: what that download is of; see partState
	tmpSuffix       = ".tmp"  // a JSON document mid-write
	oldSuffix       = ".old"  // renamed aside because it could not be deleted; see removeFile
	partStateSuffix = partSuffix + stateSuffix
)

// File is one installed file, with its path relative to the dependency's destination and always slash-separated so a
// manifest written on Windows reads the same everywhere.
type File struct {
	Path string `json:"path"`
	Size int64  `json:"size"`

	// Sha256 is recorded only for a file downloaded directly, where the hash was computed in-stream and cost nothing.
	// It is absent for anything extracted from an archive - see recordTree - and nothing reads it back; it is there to
	// be read by a human diagnosing an install.
	Sha256 string `json:"sha256,omitempty"`
}

// Manifest records what a dependency put on disk.
type Manifest struct {
	Schema int `json:"schema"`

	// Name and Version are written for whoever opens the file, not for this package: nothing reads them back, and
	// Fingerprint below is what any decision is made on. They are what makes a manifest identifiable by hand, which is
	// the only way to tell whose record a stray `.something.json` in models/ is.
	Name    string `json:"name"`
	Version string `json:"version"`

	// Fingerprint, not Version, is what decides whether to reinstall. It covers the release tag and every source URL
	// and expected hash, so both kinds of change are caught by one comparison: an archive dependency moves when its
	// tag is bumped, and a model moves when its hash changes upstream, which no tag would have recorded. A model with
	// no known hash fingerprints from its URL alone and therefore never churns.
	Fingerprint string `json:"fingerprint"`

	Files []File `json:"files"`
}

// fingerprint derives the reinstall key of a dependency from the inputs that define its content.
func fingerprint(dep Dependency) string {
	h := sha256.New()
	h.Write([]byte(dep.Version))

	for _, src := range dep.Sources {
		h.Write([]byte{0})
		h.Write([]byte(src.URL))
		h.Write([]byte{0})
		h.Write([]byte(src.Sha256))
	}

	return hex.EncodeToString(h.Sum(nil))
}

// readManifest loads the record from dir, reporting false when there is none to read. A manifest that is missing,
// unreadable, malformed or written by another schema all mean the same thing to the caller - there is nothing to trust
// here - so they collapse into one answer instead of an error that every call site would have to ignore.
func readManifest(dir, name string) (Manifest, bool) {
	data, err := os.ReadFile(filepath.Join(dir, name))
	if err != nil {
		return Manifest{}, false
	}

	var m Manifest
	if err = json.Unmarshal(data, &m); err != nil {
		internal.Log().Warn("ignoring a malformed manifest", "dir", dir, "manifest", name, "err", err)
		return Manifest{}, false
	}

	if m.Schema != manifestSchema {
		internal.Log().Info("ignoring a manifest from another schema",
			"dir", dir, "manifest", name, "schema", m.Schema)
		return Manifest{}, false
	}

	return m, true
}

// writeManifest saves the record atomically, so a manifest is only ever observed complete: an interrupted write leaves
// the previous one, or none at all, and both mean "reinstall" rather than "trust a truncated file list".
func writeManifest(dir, name string, m Manifest) error {
	return internal.WriteJSONAtomic(dir, name, m)
}

// intact reports whether every recorded file is still present at its recorded size. This is the steady-state check that
// runs on each start, so it is deliberately a stat per file rather than a hash: re-reading a few gigabytes of NVIDIA
// libraries on every launch would cost seconds to catch a corruption that the install-time hashing already ruled out.
//
// A file the manifest doesn't name is ignored. That matters for a shared destination like models/, where another
// dependency's files sit alongside these ones.
func (m Manifest) intact(dir string) bool {
	if len(m.Files) == 0 {
		return false
	}

	for _, f := range m.Files {
		info, err := os.Stat(filepath.Join(dir, filepath.FromSlash(f.Path)))
		if err != nil || info.IsDir() || info.Size() != f.Size {
			return false
		}
	}

	return true
}

// remove deletes exactly the recorded files, then prunes the subdirectories that emptied. dir itself always survives:
// on Linux the library directories are on LD_LIBRARY_PATH from before the process re-execed, and the loader
// permanently skips a search path it finds missing.
func (m Manifest) remove(dir string) error {
	for _, f := range m.Files {
		if err := removeFile(filepath.Join(dir, filepath.FromSlash(f.Path))); err != nil {
			return err
		}
	}

	// Deepest first, so a nested directory is gone before its parent is tried. os.Remove fails on a directory that
	// still holds something, which is exactly the wanted behaviour, so the error is the stop condition rather than a
	// failure.
	for _, sub := range subdirs(m.Files) {
		os.Remove(filepath.Join(dir, filepath.FromSlash(sub)))
	}

	return nil
}

// removeFile deletes one installed file, falling back to renaming it aside when it cannot be deleted.
//
// The fallback is for Windows, where a DLL mapped into a running process cannot be removed but can still be renamed. A
// second instance of the app holding the ONNX Runtime open would otherwise make an upgrade impossible; renaming frees
// the name for the new file and leaves the old one to be swept on the next install.
func removeFile(path string) error {
	err := os.Remove(path)
	if err == nil || os.IsNotExist(err) {
		return nil
	}

	if renameErr := os.Rename(path, path+oldSuffix); renameErr == nil {
		internal.Log().Warn("could not delete a file in use; renamed it aside", "path", path)
		return nil
	}

	return errors.Wrapf(err, "failed to remove %s; close any other running instance of the app", path)
}

// subdirs returns every directory the files live in, deepest first.
func subdirs(files []File) []string {
	seen := make(map[string]struct{})

	for _, f := range files {
		for d := path.Dir(f.Path); d != "." && d != "/"; d = path.Dir(d) {
			seen[d] = struct{}{}
		}
	}

	dirs := make([]string, 0, len(seen))
	for d := range seen {
		dirs = append(dirs, d)
	}

	sort.Slice(dirs, func(i, j int) bool {
		return strings.Count(dirs[i], "/") > strings.Count(dirs[j], "/")
	})

	return dirs
}

// recordTree records every file under dir, which is how an archive's contents are captured: extraction produces a list
// nobody declared, so the list is read back off the disk afterwards.
//
// Only the path and size are recorded. Hashing each extracted file would be a second full read of the expanded tree -
// TensorRT alone is 1.78 GB compressed and more on disk - to produce a value nothing reads: intact compares sizes, and
// deliberately so, while the bytes as downloaded were already verified in-stream against the pinned hash of the
// archive they came out of, which is the check that actually establishes the contents are genuine.
//
// Enumerating the result rather than diffing the directory before and after is deliberate - an archive that overwrites
// a file already present would be missing from a diff, and the manifest has to name every file it owns for the next
// uninstall to be exact.
//
// The walk is done here rather than through fs.ListPath because that returns paths alone, discarding the DirEntry it
// already walked with - and the size is on that entry, served from the same readdir batch. Asking for it again is a
// second lstat per file, which on a CUDA tree of several hundred is a directory walk's worth of syscalls for
// information already in hand.
func recordTree(dir, manifest string) ([]File, error) {
	var files []File

	err := filepath.WalkDir(dir, func(p string, d os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return errors.Wrapf(walkErr, "failed to walk %s", p)
		}

		if d.IsDir() {
			return nil
		}

		rel, err := filepath.Rel(dir, p)
		if err != nil {
			return errors.Wrapf(err, "failed to relativise %s", p)
		}

		rel = filepath.ToSlash(rel)
		if rel == manifest || isTransient(rel) {
			return nil
		}

		info, err := d.Info()
		if err != nil {
			return errors.Wrapf(err, "failed to stat %s", p)
		}

		files = append(files, File{Path: rel, Size: info.Size()})
		return nil
	})
	if err != nil {
		return nil, err
	}

	sort.Slice(files, func(i, j int) bool { return files[i].Path < files[j].Path })

	return files, nil
}

// isTransient reports whether a path is bookkeeping rather than installed content: a partial download, the record of
// what that download is of, a JSON document being written, or a file renamed aside because Windows had it open.
//
// partStateSuffix has to be listed in its own right. It ends in ".json", not ".part", so a suffix match on the latter
// misses it - and a sidecar left in an exclusive destination would be walked up by recordTree and written into the
// manifest as though it were installed content.
func isTransient(rel string) bool {
	base := path.Base(rel)

	for _, suffix := range []string{partSuffix, partStateSuffix, tmpSuffix, oldSuffix} {
		if strings.HasSuffix(base, suffix) {
			return true
		}
	}

	return false
}
