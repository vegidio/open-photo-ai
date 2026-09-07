package main

import (
	"image"
	"math"
	"slices"
	"testing"
	"time"
)

// computeStats is the whole numeric output of the benchmark harness: every figure a tuning decision is made from comes
// out of here. It is also pure, so there is no reason for it to be exercised only by running a real model.

func TestComputeStatsWithNoRuns(t *testing.T) {
	got := computeStats(nil)

	if got != (stats{}) {
		t.Errorf("computeStats(nil) = %+v, want the zero value", got)
	}
}

// A single run has no spread to measure. The Bessel correction divides by n-1, so this is the case that would divide
// by zero and produce NaN if the guard were dropped.
func TestComputeStatsWithOneRun(t *testing.T) {
	got := computeStats([]time.Duration{100 * time.Millisecond})

	if got.stdDev != 0 {
		t.Errorf("stdDev = %v, want 0 for a single run", got.stdDev)
	}

	for name, d := range map[string]time.Duration{"min": got.min, "max": got.max, "mean": got.mean, "median": got.median} {
		if d != 100*time.Millisecond {
			t.Errorf("%s = %v, want 100ms", name, d)
		}
	}
}

// An even count averages the two middle values; an odd count takes the middle one. The even branch is the one that is
// easy to get wrong by an index, and it is the median that feeds megapixelsPerSecond.
func TestComputeStatsMedian(t *testing.T) {
	tests := []struct {
		name string
		runs []time.Duration
		want time.Duration
	}{
		{"odd count takes the middle", []time.Duration{10, 20, 30}, 20},
		{"even count averages the two middle", []time.Duration{10, 20, 30, 40}, 25},
		{"even count with a fractional mean rounds", []time.Duration{10, 21, 30, 40}, 26},
		{"two runs average", []time.Duration{10, 30}, 20},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := computeStats(tt.runs).median; got != tt.want {
				t.Errorf("median = %v, want %v", got, tt.want)
			}
		})
	}
}

// The sample (n-1) deviation, not the population (n) one. The comment above computeStats argues for it specifically
// because at n=5 the population formula understates the spread by ~11%, so the distinction is the point.
func TestComputeStatsUsesSampleStdDev(t *testing.T) {
	runs := []time.Duration{
		10 * time.Millisecond,
		20 * time.Millisecond,
		30 * time.Millisecond,
		40 * time.Millisecond,
		50 * time.Millisecond,
	}

	// mean 30ms; squared deviations 400+100+0+100+400 = 1000 (ms²).
	// Sample: sqrt(1000/4) = sqrt(250) ~= 15.811ms. Population would be sqrt(1000/5) = sqrt(200) ~= 14.142ms.
	wantSample := time.Duration(math.Round(math.Sqrt(250) * float64(time.Millisecond)))
	population := time.Duration(math.Round(math.Sqrt(200) * float64(time.Millisecond)))

	got := computeStats(runs).stdDev

	if got != wantSample {
		t.Errorf("stdDev = %v, want %v (Bessel-corrected)", got, wantSample)
	}

	if got == population {
		t.Error("stdDev matches the population formula, want the sample one")
	}
}

// The caller prints the runs in execution order under --verbose, so the sorted copy must not be the caller's slice.
func TestComputeStatsDoesNotReorderTheInput(t *testing.T) {
	runs := []time.Duration{30, 10, 20}
	original := slices.Clone(runs)

	computeStats(runs)

	if !slices.Equal(runs, original) {
		t.Errorf("computeStats reordered its input to %v, want %v left untouched", runs, original)
	}
}

func TestComputeStatsMinMaxAndMean(t *testing.T) {
	runs := []time.Duration{30, 10, 20, 40}

	got := computeStats(runs)

	if got.min != 10 {
		t.Errorf("min = %v, want 10", got.min)
	}

	if got.max != 40 {
		t.Errorf("max = %v, want 40", got.max)
	}

	if got.mean != 25 {
		t.Errorf("mean = %v, want 25", got.mean)
	}
}

// Throughput is input megapixels over the median. A non-positive median means there is nothing to divide by, and
// returning 0 rather than +Inf is what keeps the report readable.
func TestMegapixelsPerSecond(t *testing.T) {
	// A 1000x1000 image is exactly 1 MP, so a one-second median is 1 MP/s.
	oneMegapixel := image.Rect(0, 0, 1000, 1000)

	if got := megapixelsPerSecond(oneMegapixel, time.Second); got != 1 {
		t.Errorf("megapixelsPerSecond(1MP, 1s) = %v, want 1", got)
	}

	if got := megapixelsPerSecond(oneMegapixel, 500*time.Millisecond); got != 2 {
		t.Errorf("megapixelsPerSecond(1MP, 500ms) = %v, want 2", got)
	}

	// Scales with area, not with an edge: doubling both edges is 4x the pixels.
	if got := megapixelsPerSecond(image.Rect(0, 0, 2000, 2000), time.Second); got != 4 {
		t.Errorf("megapixelsPerSecond(4MP, 1s) = %v, want 4", got)
	}
}

func TestMegapixelsPerSecondWithNoDuration(t *testing.T) {
	oneMegapixel := image.Rect(0, 0, 1000, 1000)

	for _, median := range []time.Duration{0, -time.Second} {
		if got := megapixelsPerSecond(oneMegapixel, median); got != 0 {
			t.Errorf("megapixelsPerSecond(1MP, %v) = %v, want 0", median, got)
		}
	}
}
