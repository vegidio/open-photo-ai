package osaka

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

func TestEstimateActivationBytesIsMonotonic(t *testing.T) {
	sizes := []int{256, 512, 1024, 2048, 4096}
	var previous int64

	for _, s := range sizes {
		got := estimateActivationBytes(s, s)

		if got <= previous {
			t.Fatalf("%dx%d estimated %d, not more than the previous %d", s, s, got, previous)
		}
		previous = got
	}

	if estimateActivationBytes(0, 100) != 0 || estimateActivationBytes(100, -1) != 0 {
		t.Fatal("a degenerate size must estimate 0")
	}
}

// The region must be a multiple of the model's alignment, or every tile would be rejected.
func TestRegionEdgeIsAligned(t *testing.T) {
	if ditRegionEdge%alignment != 0 {
		t.Fatalf("region %d is not a multiple of %d", ditRegionEdge, alignment)
	}
	if tileOverlap%alignment != 0 {
		t.Fatalf("overlap %d is not a multiple of %d, so shifted tiles would misalign", tileOverlap, alignment)
	}
	if tileOverlap >= ditRegionEdge {
		t.Fatalf("overlap %d leaves no forward progress against region %d", tileOverlap, ditRegionEdge)
	}
}

// A generous budget is not flagged; a tiny one is. The region size is dictated by the graph either way, so this only
// ever affects whether the user is warned.
func TestMemoryLooksTight(t *testing.T) {
	if _, tight := memoryLooksTight(types.MemoryPoolDevice, int64(24)<<30); tight {
		t.Fatal("24 GiB flagged as tight for one region")
	}
	if _, tight := memoryLooksTight(types.MemoryPoolHost, 1<<20); !tight {
		t.Fatal("1 MiB not flagged as tight")
	}
	// An unknown budget cannot be judged, so it must not be reported as a problem.
	if _, tight := memoryLooksTight(types.MemoryPoolHost, 0); tight {
		t.Fatal("an unknown budget was flagged as tight")
	}
}

// The tiling must only ever produce regions the model accepts. This pins the relationship between the grid geometry
// and the one size the DiT takes, so a change to either is caught here rather than at inference time.
func TestEveryTileIsExactlyTheRegionSize(t *testing.T) {
	sizes := [][2]int{{512, 512}, {640, 480}, {1280, 1280}, {3000, 2000}, {4001, 733}}

	for _, s := range sizes {
		w, h := max(ditRegionEdge, utils.RoundUpTo16(s[0])), max(ditRegionEdge, utils.RoundUpTo16(s[1]))
		grid := utils.TileGrid{Size: ditRegionEdge, Overlap: tileOverlap, Width: w, Height: h}

		for _, r := range grid.Tiles() {
			if r.Dx() != ditRegionEdge || r.Dy() != ditRegionEdge {
				t.Fatalf("target %dx%d (padded %dx%d) produced a %dx%d tile", s[0], s[1], w, h, r.Dx(), r.Dy())
			}
		}
	}
}
