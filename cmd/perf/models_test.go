package main

import (
	"slices"
	"strings"
	"testing"

	opai "github.com/vegidio/open-photo-ai"
)

// The benchmark catalog and opai's model table are two lists of the same models in two modules, each holding a
// different function per model, so neither can be derived from the other. Nothing in the compiler connects them, which
// is how a newly added model ends up silently unbenchmarked. This is that connection.
//
// The catalog keys on the bare codename because that is what someone types on the command line ("perftest run tokyo");
// opai keys on "<type>_<codename>". Comparing the codename halves is what makes the two comparable.
func TestCatalogMatchesLibrary(t *testing.T) {
	want := make([]string, 0)
	for _, key := range opai.ModelKeys() {
		_, codename, found := strings.Cut(key, "_")
		if !found {
			t.Fatalf("opai model key %q is not <type>_<codename>", key)
		}

		want = append(want, codename)
	}
	slices.Sort(want)

	got := make([]string, 0, len(catalog))
	for _, e := range catalog {
		got = append(got, e.name)
	}
	slices.Sort(got)

	if !slices.Equal(got, want) {
		t.Errorf("catalog benchmarks %v, but opai builds %v", got, want)
	}
}

// The catalog is ordered, and both the sweep and `list` follow that order, so a duplicate would run twice and list
// twice rather than being caught by lookup.
func TestCatalogHasNoDuplicates(t *testing.T) {
	seen := make(map[string]bool, len(catalog))

	for _, e := range catalog {
		if seen[e.name] {
			t.Errorf("catalog lists %q more than once", e.name)
		}

		seen[e.name] = true
	}
}
