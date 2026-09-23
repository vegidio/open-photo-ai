package services

import (
	"slices"
	"strings"
	"testing"
)

func TestFilterPatternsIncludesUpperCase(t *testing.T) {
	got := strings.Split(filterPatterns([]string{"jpg", "dng", "3fr"}), ";")
	want := []string{"*.jpg", "*.JPG", "*.dng", "*.DNG", "*.3fr", "*.3FR"}

	if !slices.Equal(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}
