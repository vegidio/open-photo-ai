package internal

import (
	"path/filepath"
	"strings"
	"testing"
)

// The traversal cases are the ones that matter: a model id reaches EngineCacheFor from outside the process, and the
// directory ConfigDir returns is handed to deps.EmptyDir, which removes everything in it.
func TestConfigDirRejectsTraversal(t *testing.T) {
	SetAppName("opai-configdir-test")

	rejected := []string{
		"engines/../../..",
		"engines/dn_stockholm_../../../../foo",
		"engines/..",
		"../engines",
		"engines//sub",
		"engines/.",
		`engines/dn_stockholm_..\..\..`,
		`engines/a\b`,
		"",
	}

	for _, rel := range rejected {
		if _, err := ConfigDir(rel); err == nil {
			t.Errorf("ConfigDir(%q) was accepted; it must be rejected", rel)
		}
	}
}

func TestConfigDirAcceptsOrdinaryPaths(t *testing.T) {
	SetAppName("opai-configdir-test")

	for _, rel := range []string{"models", "engines", EngineCacheFor("dn_stockholm_fp32"), TimingCacheDir} {
		dir, err := ConfigDir(rel)
		if err != nil {
			t.Fatalf("ConfigDir(%q) failed: %v", rel, err)
		}

		// The returned path must actually end in the requested segments, or the check above is guarding something
		// other than what callers get back.
		want := filepath.FromSlash(rel)
		if !strings.HasSuffix(dir, want) {
			t.Errorf("ConfigDir(%q) = %q, which does not end in %q", rel, dir, want)
		}
	}
}
