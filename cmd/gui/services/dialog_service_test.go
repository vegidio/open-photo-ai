package services

import (
	"slices"
	"strings"
	"testing"
)

func TestFilterPatternsMatchesAnyCaseOnLinux(t *testing.T) {
	got := strings.Split(filterPatterns([]string{"jpg", "dng", "3fr"}, "linux"), ";")
	want := []string{"*.[jJ][pP][gG]", "*.[dD][nN][gG]", "*.3[fF][rR]"}

	if !slices.Equal(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func TestFilterPatternsIsPlainElsewhere(t *testing.T) {
	got := strings.Split(filterPatterns([]string{"jpg", "dng", "3fr"}, "darwin"), ";")
	want := []string{"*.jpg", "*.dng", "*.3fr"}

	if !slices.Equal(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}
