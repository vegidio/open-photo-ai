package utils

import (
	"image"
	"testing"
)

// applyOpts mirrors what RunTiledInference does: seed the defaults, then let the options override them.
func applyOpts(opts ...TileOption) tileConfig {
	cfg := tileConfig{size: DefaultTileSize, overlap: DefaultTileOverlap}
	for _, opt := range opts {
		opt(&cfg)
	}

	return cfg
}

func TestTileGeometryDefaults(t *testing.T) {
	cfg := applyOpts()

	if cfg.size != DefaultTileSize || cfg.overlap != DefaultTileOverlap {
		t.Fatalf("no option must keep %d/%d, got %d/%d",
			DefaultTileSize, DefaultTileOverlap, cfg.size, cfg.overlap)
	}

	// The divergence guard must not disturb the geometry: the models that use it rely on the defaults.
	if cfg = applyOpts(WithDivergenceGuard(10)); cfg.size != DefaultTileSize || cfg.overlap != DefaultTileOverlap {
		t.Fatalf("divergence guard changed the geometry: %d/%d", cfg.size, cfg.overlap)
	}
}

func TestWithTileGeometry(t *testing.T) {
	tests := []struct {
		name            string
		size, overlap   int
		wantSize, wantO int
	}{
		{"applied", 1024, 128, 1024, 128},
		{"zero overlap is valid", 512, 0, 512, 0},
		{"non-positive size ignored", 0, 16, DefaultTileSize, DefaultTileOverlap},
		{"negative size ignored", -1, 16, DefaultTileSize, DefaultTileOverlap},
		{"negative overlap ignored", 512, -1, DefaultTileSize, DefaultTileOverlap},
		{"overlap equal to size ignored", 512, 512, DefaultTileSize, DefaultTileOverlap},
		{"overlap larger than size ignored", 512, 600, DefaultTileSize, DefaultTileOverlap},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := applyOpts(WithTileGeometry(tt.size, tt.overlap))

			if cfg.size != tt.wantSize || cfg.overlap != tt.wantO {
				t.Fatalf("want %d/%d, got %d/%d", tt.wantSize, tt.wantO, cfg.size, cfg.overlap)
			}
		})
	}
}

func TestTileGridCoversEveryPixel(t *testing.T) {
	tests := []struct {
		name                       string
		w, h, size, overlap, wantN int
	}{
		{"single tile when the image fits", 200, 200, 256, 16, 1},
		{"stride, not tile size, sets the step", 480, 240, 256, 16, 2},
		{"ragged edges", 1000, 700, 256, 16, 15},
		{"large osaka-style tiles", 3000, 2000, 1024, 128, 12},
		{"invalid geometry falls back to one tile", 500, 500, 0, 0, 1},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			g := TileGrid{Size: tt.size, Overlap: tt.overlap, Width: tt.w, Height: tt.h}
			tiles := g.Tiles()

			if len(tiles) != tt.wantN {
				t.Fatalf("want %d tiles, got %d", tt.wantN, len(tiles))
			}
			if g.Count() != len(tiles) {
				t.Fatalf("Count()=%d disagrees with len(Tiles())=%d", g.Count(), len(tiles))
			}

			// Every pixel must be covered by at least one tile, and no tile may leave the image.
			covered := make([]bool, tt.w*tt.h)
			bounds := image.Rect(0, 0, tt.w, tt.h)

			for _, tile := range tiles {
				if !tile.In(bounds) {
					t.Fatalf("tile %v escapes the image %v", tile, bounds)
				}
				if tile.Empty() {
					t.Fatalf("empty tile %v", tile)
				}

				for y := tile.Min.Y; y < tile.Max.Y; y++ {
					for x := tile.Min.X; x < tile.Max.X; x++ {
						covered[y*tt.w+x] = true
					}
				}
			}

			for i, ok := range covered {
				if !ok {
					t.Fatalf("pixel (%d,%d) is not covered by any tile", i%tt.w, i/tt.w)
				}
			}
		})
	}
}

func TestTileGridEmptyImage(t *testing.T) {
	for _, g := range []TileGrid{
		{Size: 256, Overlap: 16, Width: 0, Height: 100},
		{Size: 256, Overlap: 16, Width: 100, Height: 0},
	} {
		if tiles := g.Tiles(); tiles != nil {
			t.Fatalf("want no tiles for %+v, got %d", g, len(tiles))
		}
		if n := g.Count(); n != 0 {
			t.Fatalf("want count 0 for %+v, got %d", g, n)
		}
	}
}

// The last tile is shifted back to sit flush with the image rather than shrunk. When that shift lands it exactly on
// top of the previous tile, emitting both means running the model twice on identical pixels - cheap for a
// convolutional model, but tens of seconds per duplicate for a diffusion one.
func TestTileGridDropsCollapsedEdgeTiles(t *testing.T) {
	tests := []struct {
		name                     string
		length, size, overlap, n int
	}{
		{"the shifted tile duplicates the previous one", 1280, 512, 128, 3},
		{"an exact fit needs no shift", 1024, 512, 0, 2},
		{"a genuine partial tile is kept", 1300, 512, 128, 4},
		{"image smaller than a tile", 300, 512, 128, 1},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			g := TileGrid{Size: tt.size, Overlap: tt.overlap, Width: tt.length, Height: tt.size}

			if got := len(g.offsets(tt.length)); got != tt.n {
				t.Fatalf("want %d offsets, got %d (%v)", tt.n, got, g.offsets(tt.length))
			}

			// Dropping a tile must never uncover a pixel.
			covered := make([]bool, tt.length)
			for _, x := range g.offsets(tt.length) {
				for i := x; i < min(x+g.extent(tt.length), tt.length); i++ {
					covered[i] = true
				}
			}
			for i, ok := range covered {
				if !ok {
					t.Fatalf("position %d uncovered", i)
				}
			}
		})
	}
}

// No two tiles may share a position, whatever the geometry.
func TestTileGridHasNoDuplicates(t *testing.T) {
	for _, size := range []int{256, 512, 1024} {
		for _, overlap := range []int{0, 16, 128} {
			for _, w := range []int{300, 512, 1000, 1280, 1536, 3000} {
				g := TileGrid{Size: size, Overlap: overlap, Width: w, Height: w}
				seen := map[[2]int]bool{}

				for _, r := range g.Tiles() {
					key := [2]int{r.Min.X, r.Min.Y}
					if seen[key] {
						t.Fatalf("size=%d overlap=%d w=%d emitted %v twice", size, overlap, w, key)
					}
					seen[key] = true
				}

				if g.Count() != len(g.Tiles()) {
					t.Fatalf("size=%d overlap=%d w=%d: Count()=%d but %d tiles", size, overlap, w, g.Count(), len(g.Tiles()))
				}
			}
		}
	}
}
