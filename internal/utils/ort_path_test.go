package utils

import (
	"os"
	"path/filepath"
	"testing"
)

// TestIsASCIIPath covers the encoding boundary the TensorRT cache paths are checked against. The non-ASCII cases are
// the two that actually reached telemetry - a Cyrillic and a Latin-1 profile directory - because both are outside
// ASCII while only one of them looks it.
func TestIsASCIIPath(t *testing.T) {
	tests := []struct {
		name string
		path string
		want bool
	}{
		{"ascii profile", `C:\Users\jase\AppData\Roaming\open-photo-ai\engines\dt_newyork_fp32`, true},
		{"empty", "", true},
		{"short name", `C:\Users\ROMAN~1\AppData\Roaming\OPEN-P~1\engines`, true},
		{"cyrillic profile", `C:\Users\Роман\AppData\Roaming\open-photo-ai\engines\dt_newyork_fp32`, false},
		{"accented latin profile", `C:\Users\Vladimír\AppData\Roaming\open-photo-ai\engines`, false},
		{"non-ascii in the last segment only", `C:\Users\jase\engines\dt_newyörk_fp32`, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isASCIIPath(tt.path); got != tt.want {
				t.Errorf("isASCIIPath(%q) = %v, want %v", tt.path, got, tt.want)
			}
		})
	}
}

// TestApplyTrtCachePathsEnablesAsciiPaths pins the ordinary case: both directories are usable, so both caches are on
// and pointed at them.
func TestApplyTrtCachePathsEnablesAsciiPaths(t *testing.T) {
	paths := cachePaths{engine: asciiDir(t, "engines"), timing: asciiDir(t, "timing")}

	options := map[string]string{}
	applyTrtCachePaths(options, paths)

	if options["trt_engine_cache_enable"] != "1" {
		t.Errorf("engine cache = %q, want enabled", options["trt_engine_cache_enable"])
	}
	if options["trt_engine_cache_path"] != paths.engine {
		t.Errorf("engine path = %q, want %q", options["trt_engine_cache_path"], paths.engine)
	}
	if options["trt_timing_cache_enable"] != "1" {
		t.Errorf("timing cache = %q, want enabled", options["trt_timing_cache_enable"])
	}
	if options["trt_timing_cache_path"] != paths.timing {
		t.Errorf("timing path = %q, want %q", options["trt_timing_cache_path"], paths.timing)
	}
}

// TestApplyTrtCachePathsNeverLeavesAPathWithoutItsFlag is the invariant that matters regardless of platform: a cache
// is never left enabled while pointing nowhere, and a path is never set for a cache that is off. Handing ORT an
// enabled cache with no path is what turns a cache problem into a failed session.
func TestApplyTrtCachePathsNeverLeavesAPathWithoutItsFlag(t *testing.T) {
	for _, dir := range []string{
		asciiDir(t, "engines"),
		`C:\Users\Роман\AppData\Roaming\open-photo-ai\engines\dt_newyork_fp32`,
	} {
		options := map[string]string{}
		applyTrtCachePaths(options, cachePaths{engine: dir, timing: dir})

		for _, cache := range []string{"engine", "timing"} {
			enabled := options["trt_"+cache+"_cache_enable"]
			path, hasPath := options["trt_"+cache+"_cache_path"]

			if enabled != "0" && enabled != "1" {
				t.Fatalf("%s: %s cache flag = %q, want \"0\" or \"1\"", dir, cache, enabled)
			}
			if enabled == "1" && (!hasPath || path == "") {
				t.Errorf("%s: %s cache is enabled with no path", dir, cache)
			}
			if enabled == "0" && hasPath {
				t.Errorf("%s: %s cache is disabled but still carries path %q", dir, cache, path)
			}
		}
	}
}

// asciiDir returns a real directory that is guaranteed ASCII, so the test does not depend on whether the machine
// running it has a non-ASCII user name. It has to exist because the Windows implementation resolves it against the
// filesystem.
func asciiDir(t *testing.T, name string) string {
	t.Helper()

	// t.TempDir() is rooted in the user's temp directory, which on a Windows machine with a non-ASCII profile is
	// itself non-ASCII - the very case this would be trying to hold constant.
	root := t.TempDir()
	if !isASCIIPath(root) {
		t.Skipf("the temp directory %q is not ASCII, so it cannot stand in for an ASCII cache directory", root)
	}

	// Created rather than merely named because the Windows implementation resolves the path against the filesystem.
	dir := filepath.Join(root, name)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("failed to create %q: %v", dir, err)
	}

	return dir
}
