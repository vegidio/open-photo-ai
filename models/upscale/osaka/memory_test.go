package osaka

import (
	"math"
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

func TestEstimateActivationBytesIsMonotonic(t *testing.T) {
	sizes := []int{256, 512, 1024, 2048, 4096}
	var previous int64

	for _, s := range sizes {
		got := EstimateActivationBytes(s, s)

		if got <= previous {
			t.Fatalf("%dx%d estimated %d, not more than the previous %d", s, s, got, previous)
		}
		previous = got
	}

	if EstimateActivationBytes(0, 100) != 0 || EstimateActivationBytes(100, -1) != 0 {
		t.Fatal("a degenerate size must estimate 0")
	}
}

// The region size is dictated by the graph, not by memory, so it must be the same whatever the pool reports - and
// it must always be the one size the DiT actually accepts.
func TestRegionSizeIsAlwaysTheOnlySizeTheDitAccepts(t *testing.T) {
	budgets := []int64{0, 1, 1 << 20, 1 << 30, int64(math.MaxInt64 / 4)}

	for _, pool := range []types.MemoryPool{types.MemoryPoolHost, types.MemoryPoolDevice} {
		for _, b := range budgets {
			edge, _ := RegionSize(pool, b)

			if edge != ditRegionEdge {
				t.Fatalf("pool=%v available=%d gave region %d, want %d", pool, b, edge, ditRegionEdge)
			}
		}
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

// A generous budget is reported as fitting; a tiny one is not. The size is unchanged either way.
func TestRegionSizeReportsWhetherItFits(t *testing.T) {
	if _, ok := RegionSize(types.MemoryPoolDevice, int64(24)<<30); !ok {
		t.Fatal("24 GiB reported as not fitting one 512x512 region")
	}
	if _, ok := RegionSize(types.MemoryPoolHost, 1<<20); ok {
		t.Fatal("1 MiB reported as fitting")
	}
	// An unknown budget cannot be judged, so it must not be reported as a problem.
	if _, ok := RegionSize(types.MemoryPoolHost, 0); !ok {
		t.Fatal("an unknown budget was reported as not fitting")
	}
}

func TestAlignUp(t *testing.T) {
	tests := map[int]int{0: 0, 1: 16, 15: 16, 16: 16, 17: 32, 640: 640, 641: 656, 3000: 3008}

	for in, want := range tests {
		if got := alignUp(in); got != want {
			t.Fatalf("alignUp(%d) = %d, want %d", in, got, want)
		}
	}
}

// The tiling must only ever produce regions the model accepts. This pins the relationship between the grid geometry
// and the one size the DiT takes, so a change to either is caught here rather than at inference time.
func TestEveryTileIsExactlyTheRegionSize(t *testing.T) {
	sizes := [][2]int{{512, 512}, {640, 480}, {1280, 1280}, {3000, 2000}, {4001, 733}}

	for _, s := range sizes {
		w, h := max(ditRegionEdge, alignUp(s[0])), max(ditRegionEdge, alignUp(s[1]))
		grid := utils.TileGrid{Size: ditRegionEdge, Overlap: tileOverlap, Width: w, Height: h}

		for _, r := range grid.Tiles() {
			if r.Dx() != ditRegionEdge || r.Dy() != ditRegionEdge {
				t.Fatalf("target %dx%d (padded %dx%d) produced a %dx%d tile", s[0], s[1], w, h, r.Dx(), r.Dy())
			}
		}
	}
}
