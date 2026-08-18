// Package deps installs the files the application downloads at runtime - the ONNX Runtime, the NVIDIA libraries and the
// models - and keeps a record of what each one put on disk.
//
// The record is what makes a multi-file dependency checkable. A `.7z` holding a runtime plus its execution providers
// used to be verified by hashing a single file inside it, so a truncated provider was invisible; here the archive is
// verified as a whole against a pinned hash before it is expanded, and every file it produced is then written to a
// manifest. That manifest is also the list of files to delete when the dependency is replaced, which is the only way to
// stop a previous version's shared libraries from lingering next to a new one.
package deps

import "path"

// ManifestName is the record a dependency writes when it owns its whole directory. A destination shared by several
// dependencies - models/ - names its manifests after the dependency instead, so two concurrent installs never write to
// the same file.
const ManifestName = ".manifest.json"

// Source is one remote artifact a dependency is installed from.
type Source struct {
	// URL is the absolute address to download from. Its base name is the file name written to disk.
	URL string

	// Sha256 is the expected hash of the bytes as downloaded - of the `.7z` itself, not of anything inside it, so a
	// single pinned value covers every file the archive carries. An empty value means no pinned hash is available,
	// which happens for a model when the remote manifest couldn't be loaded, and installs the artifact unverified.
	Sha256 string

	// Size is the expected size in bytes, or 0 when unknown. It is only used to spread a progress report across the
	// sources of a multi-file dependency, so a model split into a graph and a weights blob reports one 0-100% run
	// instead of restarting at the second file.
	Size int64
}

// FileName is the name this source is written to disk under. It is a method rather than three call sites deriving it,
// because the rule above - "the base name of the URL" - has to hold for all of them at once: whatever validate
// inspects, fetch writes and sourcesPresent looks for must be the same file.
func (s Source) FileName() string {
	return path.Base(s.URL)
}

// Dependency is an installable set of files described by one manifest.
type Dependency struct {
	// Name identifies the dependency in logs and in its manifest: "onnx-runtime", "cuda", "up_kyoto_4x_fp32".
	Name string

	// Version is the release tag the sources came from. It is empty for models, which are versioned by the hashes in
	// the remote manifest rather than by a tag; Fingerprint is what actually decides a reinstall either way.
	Version string

	// Destination is a slash-separated path under the user's config directory: "runtime", "libs/cuda", "models".
	Destination string

	// Manifest is the file name the record is written to, inside Destination.
	Manifest string

	Sources []Source

	// Exclusive marks a destination that belongs to this dependency alone. Only an exclusive destination may hold an
	// archive: extraction produces a file list nobody declared, and "everything under the directory" is only a correct
	// answer to "what did this install?" when nothing else writes there. It also allows a directory with no manifest
	// to be emptied before installing, which is how a tree left by an older version is replaced rather than merged.
	Exclusive bool

	// SkipVerify accepts whatever is already on disk and downgrades a hash mismatch to a warning. It carries the debug
	// override behind opai.SetSkipModelVerification, which used to be a `destination == "models"` string comparison
	// buried in the download path.
	SkipVerify bool

	// Derived lists directories, relative to the config directory, holding artifacts computed *from* this dependency
	// rather than downloaded with it: the TensorRT engine cache and the CoreML compiled model. They are removed on
	// every install, because an engine compiled from the weights being replaced is at best wasted disk and at worst
	// silently wrong. Nothing recreates them here - the execution providers rebuild their own cache on the next
	// session.
	Derived []string
}
