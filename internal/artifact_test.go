package internal

import "testing"

// pinnedPlatforms is the coverage every dependency is expected to have: the ONNX Runtime is built for all five
// platforms, the NVIDIA libraries only for the three they are published for.
var pinnedPlatforms = map[string][]string{
	"onnx":     {"darwin_arm64", "linux_amd64", "linux_arm64", "windows_amd64", "windows_arm64"},
	"cuda":     {"linux_amd64", "linux_arm64", "windows_amd64"},
	"cudnn":    {"linux_amd64", "linux_arm64", "windows_amd64"},
	"tensorrt": {"linux_amd64", "linux_arm64", "windows_amd64"},
}

// TestEveryArchiveIsPinned guards the one failure this table exists to prevent.
//
// A missing entry used to be silent: the lookup returned an empty hash, an empty hash means "do not verify", and the
// archive installed unverified with nothing saying so. That is how the NVIDIA libraries shipped unchecked for a
// release - the table held only the onnx entries, and nothing noticed the others were absent.
func TestEveryArchiveIsPinned(t *testing.T) {
	for prefix, platforms := range pinnedPlatforms {
		release, found := Releases[prefix]
		if !found {
			t.Errorf("%s is not pinned to any release", prefix)
			continue
		}

		if release.Tag == "" {
			t.Errorf("%s has no release tag, so no URL can be built for it", prefix)
		}

		for _, platform := range platforms {
			artifact, ok := release.Archives[platform]

			switch {
			case !ok:
				t.Errorf("%s has no %s archive", prefix, platform)
			case artifact.Hash == "":
				t.Errorf("%s/%s has no pinned hash, so it would install unverified", prefix, platform)
			case artifact.Size <= 0:
				t.Errorf("%s/%s has no published size", prefix, platform)
			}
		}
	}
}

// TestPinnedArchiveResolvesThisPlatform covers the resolution the installer depends on: the asset name is derived from
// the dependency and the platform, and it carries the tag its URL is built from.
func TestPinnedArchiveResolvesThisPlatform(t *testing.T) {
	pinned, found := PinnedArchive("onnx")
	if !found {
		t.Fatal("no ONNX Runtime is pinned for the platform these tests run on")
	}

	if pinned.Tag != Releases["onnx"].Tag {
		t.Errorf("tag = %q, want %q", pinned.Tag, Releases["onnx"].Tag)
	}

	if pinned.Hash == "" || pinned.Size <= 0 {
		t.Errorf("resolved an archive with nothing to verify against: %+v", pinned)
	}
}

// TestPinnedArchiveReportsAbsence covers the boolean standing between a missing entry and an unverified install.
func TestPinnedArchiveReportsAbsence(t *testing.T) {
	if _, found := PinnedArchive("openvino"); found {
		t.Error("a dependency that is not pinned must not be reported as found")
	}

	// CUDA is not published for macOS, so its entry exists but this platform's archive does not. Both are "no" to the
	// caller, and both have to be, or the installer would build a URL for an archive that was never released.
	if _, ok := Releases["cuda"].Archives["darwin_arm64"]; ok {
		t.Error("there is no macOS CUDA build; an entry for one is a mistake in the table")
	}
}

// TestReleaseTag covers the value the execution provider cache is stamped with.
func TestReleaseTag(t *testing.T) {
	if ReleaseTag("onnx") == "" {
		t.Error("the ONNX Runtime has no tag to stamp the engine cache with")
	}

	if ReleaseTag("openvino") != "" {
		t.Error("an unpinned dependency must report no tag")
	}
}
