package internal

import (
	"fmt"
	"runtime"
)

// Artifact is the pinned identity of one published archive.
type Artifact struct {
	// Hash is the SHA-256 of the `.7z` as published - of the archive itself, not of anything inside it.
	Hash string

	// Size is the published size in bytes. It is only used to report download progress before the response declares a
	// length.
	Size int64
}

// Release is one dependency pinned to one published release.
type Release struct {
	// Tag is the release the archives come from, e.g. "cuda/13.3.0". It is the version this build installs.
	Tag string

	// Archives holds the pinned identity of each platform's archive, keyed "<goos>_<goarch>". A platform with no entry
	// is one this dependency is not published for - there is no CUDA build for macOS, and none for Windows on ARM.
	Archives map[string]Artifact
}

// Pinned is everything needed to fetch and verify one dependency on the platform this binary was built for.
type Pinned struct {
	Name string // the release asset, e.g. "cuda_linux_amd64.7z"
	Tag  string // the release it belongs to, e.g. "cuda/13.3.0"
	Hash string
	Size int64
}

// PinnedArchive resolves a dependency to the archive published for the running platform, reporting false when the
// dependency is unknown or is not built for this GOOS/GOARCH.
//
// The boolean is the point of this function. The lookup it replaced returned a zero value, so a dependency missing
// from the table installed against an empty expected hash - which means "do not verify" - and nothing said so. That is
// how the NVIDIA libraries shipped unchecked for a release. There is no longer a value to install unverified against.
func PinnedArchive(prefix string) (Pinned, bool) {
	release, found := Releases[prefix]
	if !found {
		return Pinned{}, false
	}

	platform := runtime.GOOS + "_" + runtime.GOARCH

	artifact, found := release.Archives[platform]
	if !found {
		return Pinned{}, false
	}

	return Pinned{
		Name: fmt.Sprintf("%s_%s.7z", prefix, platform),
		Tag:  release.Tag,
		Hash: artifact.Hash,
		Size: artifact.Size,
	}, true
}

// ReleaseTag is the version a dependency is pinned to, empty when it isn't pinned at all. It is what stamps the
// execution provider cache, which is invalidated whenever the runtime it was compiled against moves.
func ReleaseTag(prefix string) string {
	return Releases[prefix].Tag
}
