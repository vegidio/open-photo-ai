package utils

import (
	"context"
	"image"
	"image/draw"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

const (
	// defaultTileSize and defaultTileOverlap are the geometry every fixed-shape model shipped today was tuned
	// against. They are the only geometry RunTiledInference uses; a model needing its own drives TileGrid directly.
	defaultTileSize    = 256
	defaultTileOverlap = 16
)

// TileOption configures an optional behavior of RunTiledInference.
type TileOption func(*tileConfig)

type tileConfig struct {
	divergenceThreshold float32 // <=0 disables the per-tile divergence guard
}

// WithDivergenceGuard enables the per-tile divergence guard: if a tile's RAW model output exceeds threshold in absolute
// magnitude, the original input pixels are kept for that tile (identity passthrough) instead of the diverged model
// output. This guards against models (e.g. NAFNet-based) that occasionally blow up on out-of-distribution tiles.
func WithDivergenceGuard(threshold float32) TileOption {
	return func(c *tileConfig) { c.divergenceThreshold = threshold }
}

// RunTiledInference runs a fixed-shape ONNX session over an image in overlapping 256x256 tiles and stitches the results
// back together with soft blending. scale is the model's output scale factor (1 for denoise, N for an NxN upscale), so
// the result has dimensions width*scale x height*scale.
//
// The caller is responsible for emitting the initial onProgress(0): upscale invokes this once per scale pass and
// must not reset progress to 0 on each pass.
//
// This is the tiling path a model should take. A custom driver gives up the divergence guard, the blending, the
// progress accounting and the partitioning rule all at once, and every one of those then has to be kept correct in a
// second place. Write one only when this driver genuinely cannot express what the model needs - and say in its doc
// comment what that was. Osaka is the one current exception; see Osaka.restore.
func RunTiledInference(
	ctx context.Context,
	session *Session,
	img image.Image,
	scale int,
	onProgress types.InferenceProgress,
	opts ...TileOption,
) (*image.RGBA, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	var cfg tileConfig
	for _, opt := range opts {
		opt(&cfg)
	}

	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	result := image.NewRGBA(image.Rect(0, 0, width*scale, height*scale))

	// The partitioning - including the rule that shifts an out-of-bounds tile back in rather than shrinking it - comes
	// from TileGrid, so this driver and the ones that cannot use it agree on where the tiles are by construction.
	grid := TileGrid{Size: defaultTileSize, Overlap: defaultTileOverlap, Width: width, Height: height}
	tiles := grid.Tiles()

	// Without this the loop below runs zero times and `result` - allocated but never written - is returned with a nil
	// error, handing the caller a blank image that looks like a successful enhancement. `step` would also be a
	// division by zero. There is no partitioning of a zero-area image, so this is a real failure, not a fast path.
	if len(tiles) == 0 {
		internal.Log().Warn("cannot tile the image", "width", width, "height", height)
		return nil, errors.Newf("cannot tile a %dx%d image", width, height)
	}

	columns := grid.Columns()

	// Every tile is padded out to the same fixed shape, so the input buffer and both tensors are identical from one
	// tile to the next. Built once here and reused: allocating them per tile meant ~800 KB of float32 plus two ORT
	// values for each of what can be thousands of tiles, all immediately garbage.
	scratch, err := newTileScratch(defaultTileSize, scale)
	if err != nil {
		return nil, err
	}
	defer scratch.destroy()

	step := 1 / float64(len(tiles))
	total := 0.0

	for i, rect := range tiles {
		if err := ctx.Err(); err != nil {
			return nil, errors.Wrap(err, "context cancelled")
		}

		tileX, tileY := rect.Min.X, rect.Min.Y
		tileW, tileH := rect.Dx(), rect.Dy()

		paddedTile := prepareTileForInference(img, tileX, tileY, tileW, tileH, defaultTileSize)

		processedTile, err := processTile(session, scratch, paddedTile, tileW, tileH, scale, cfg.divergenceThreshold)
		if err != nil {
			return nil, errors.Wrap(err, "failed to process tile")
		}

		// Ramp the edges that meet an already-written neighbour: every tile but the first of its row has one to the
		// left, and every tile but the first row has one above.
		blendLeft, blendTop := i%columns > 0, i >= columns

		blendTileWithOverlap(result, processedTile, tileX*scale, tileY*scale, defaultTileOverlap*scale,
			blendLeft, blendTop)

		if onProgress != nil {
			total += step
			onProgress(ClampProgress(total))
		}
	}

	return result, nil
}

// TileGrid enumerates the tiles covering a Width x Height image at a given geometry.
//
// It exists so the "shift an out-of-bounds tile back in rather than shrink it" rule has one implementation shared by
// the fixed-shape driver above and by drivers that cannot use it - notably the diffusion upscaler, which needs the
// same partitioning but a completely different per-tile pipeline.
type TileGrid struct {
	Size    int
	Overlap int
	Width   int
	Height  int
}

// Columns is how many tiles Tiles puts in each row. The driver needs it to know which tiles have a neighbour to the
// left and which have one above; deriving it here rather than at the call site keeps it from drifting out of step
// with the layout Tiles actually produces.
func (g TileGrid) Columns() int {
	return len(g.offsets(g.Width))
}

// Tiles returns the tile rectangles in row-major order.
//
// An invalid *geometry* - a non-positive tile, an overlap that leaves no forward progress, a tile bigger than what it
// covers - yields a single tile covering the whole image, so those cases can never produce an empty partitioning. A
// non-positive *dimension* is different: there is no rectangle that covers a zero-width image, so it returns nil.
// Callers must treat an empty result as an error rather than iterating zero tiles, or they will hand back an
// untouched output buffer as though the work had succeeded.
func (g TileGrid) Tiles() []image.Rectangle {
	xs := g.offsets(g.Width)
	ys := g.offsets(g.Height)

	if xs == nil || ys == nil {
		return nil
	}

	tw, th := g.extent(g.Width), g.extent(g.Height)

	tiles := make([]image.Rectangle, 0, len(xs)*len(ys))
	for _, y := range ys {
		for _, x := range xs {
			tiles = append(tiles, image.Rect(x, y, x+tw, y+th))
		}
	}

	return tiles
}

// extent is the tile size along one axis: the configured size, or the whole length when the geometry is invalid or
// the image is smaller than one tile.
func (g TileGrid) extent(length int) int {
	if !g.valid(length) {
		return length
	}

	return g.Size
}

// valid reports whether the configured geometry actually partitions an axis of the given length: a positive tile, an
// overlap that leaves forward progress, and a tile smaller than what it is covering. It is one predicate rather than
// three copies so extent and offsets cannot drift into disagreeing about what a degenerate geometry means.
func (g TileGrid) valid(length int) bool {
	return g.Size > 0 && g.Overlap >= 0 && g.Overlap < g.Size && g.Size < length
}

// offsets returns the tile start positions along one axis.
//
// The last tile is shifted back to end flush with the image rather than being shrunk, which is what keeps every tile
// the same size - a hard requirement for the fixed-shape models, and for the diffusion upscaler the only size its
// graph accepts. The shift means the final tile can land on top of the previous one; when it lands on exactly the
// same position it is dropped, because running the model twice on identical pixels is pure waste. At a 512 tile with
// 128 overlap on a 1280px side that is the difference between three tiles and four.
func (g TileGrid) offsets(length int) []int {
	if length <= 0 {
		return nil
	}
	if !g.valid(length) {
		return []int{0}
	}

	stride := g.Size - g.Overlap
	out := make([]int, 0, (length+stride-1)/stride)

	for p := 0; p < length; p += stride {
		start := min(p, length-g.Size)

		if len(out) > 0 && out[len(out)-1] == start {
			break
		}

		out = append(out, start)

		if start+g.Size >= length {
			break
		}
	}

	return out
}

// prepareTileForInference extracts a tile and reflection-pads it out to tileSize, which the fixed-shape sessions
// require even for the partial tiles at the right and bottom edges.
func prepareTileForInference(img image.Image, tileX, tileY, tileW, tileH, tileSize int) image.Image {
	tile := imaging.Crop(img, image.Rect(tileX, tileY, tileX+tileW, tileY+tileH))

	padRight := 0
	padBottom := 0

	if tileW < tileSize {
		padRight = tileSize - tileW
	}
	if tileH < tileSize {
		padBottom = tileSize - tileH
	}

	if padRight > 0 || padBottom > 0 {
		return ReflectionPad(tile, 0, 0, padRight, padBottom)
	}

	return tile
}

// tileScratch is the per-run working set for the tiled driver: one CHW input buffer and the two ORT tensors bound to
// it and to the output.
//
// The tensors are safe to reuse because ort.NewTensor wraps the Go slice's memory rather than copying it, so writing
// the next tile into input is what feeds the next Run. Every tile shares one fixed shape, which is what makes a single
// pair sufficient for the whole image.
type tileScratch struct {
	input        []float32
	inputTensor  *ort.Tensor[float32]
	outputTensor *ort.Tensor[float32]
	outW, outH   int
}

func newTileScratch(tileSize, scale int) (*tileScratch, error) {
	input := make([]float32, 3*tileSize*tileSize)

	inputTensor, err := ort.NewTensor(ort.NewShape(1, 3, int64(tileSize), int64(tileSize)), input)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}

	outW, outH := tileSize*scale, tileSize*scale

	outputTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, 3, int64(outH), int64(outW)))
	if err != nil {
		inputTensor.Destroy()
		return nil, errors.Wrap(err, "failed to create output tensor")
	}

	return &tileScratch{
		input:        input,
		inputTensor:  inputTensor,
		outputTensor: outputTensor,
		outW:         outW,
		outH:         outH,
	}, nil
}

func (s *tileScratch) destroy() {
	s.inputTensor.Destroy()
	s.outputTensor.Destroy()
}

// processTile runs the model and crops the padding back off. The crop bounds are scaled by scale to match the
// inference output dimensions (scale is 1 for denoise).
func processTile(
	session *Session,
	scratch *tileScratch,
	tile image.Image,
	tileW, tileH, scale int,
	divergenceThreshold float32,
) (image.Image, error) {
	processedTile, err := runTileInference(session, scratch, tile, scale, divergenceThreshold)
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	cropW := tileW * scale
	cropH := tileH * scale
	croppedTile := imaging.Crop(processedTile, image.Rect(0, 0, cropW, cropH))

	return croppedTile, nil
}

// runTileInference runs inference on a single padded tile. The output shares the input's shape scaled by scale (scale 1
// keeps the dimensions, e.g. denoise; scale N upscales).
func runTileInference(
	session *Session,
	scratch *tileScratch,
	tile image.Image,
	scale int,
	divergenceThreshold float32,
) (image.Image, error) {
	bounds := tile.Bounds()
	h, w := bounds.Dy(), bounds.Dx()

	// The scratch tensors are shaped for a full padded tile. Anything else means the caller skipped the padding step,
	// which would silently feed the model a stale buffer rather than this tile.
	if want := 3 * h * w; want != len(scratch.input) {
		return nil, errors.Errorf("tile is %dx%d, but the scratch buffer holds %d floats (want %d)",
			w, h, len(scratch.input), want)
	}

	// Overwrites the buffer the input tensor is bound to, which is how the next tile reaches the model.
	ImageToCHWInto(scratch.input, tile, true, false)

	err := session.Run([]ort.Value{scratch.inputTensor}, []ort.Value{scratch.outputTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	outputData := scratch.outputTensor.GetData()
	outW, outH := scratch.outW, scratch.outH

	// Per-tile divergence guard: some models numerically blow up on certain out-of-distribution tiles, exploding the
	// RAW output to ~1000+ (which clamps to a solid saturated block). Detect that from the raw values and keep the
	// original input pixels for the tile instead — one cheap max(|·|) pass, no second inference.
	if divergenceThreshold > 0 {
		for _, v := range outputData {
			if v > divergenceThreshold || v < -divergenceThreshold {
				// Diverged tile: keep the original input pixels (identity passthrough). The guard is only enabled for
				// the scale==1 path, where `tile` already matches the output dimensions; resize defensively if scale != 1.
				if scale == 1 {
					return tile, nil
				}
				return imaging.Resize(tile, w*scale, h*scale, imaging.Lanczos), nil
			}
		}
	}

	return CHWToImage(outputData, outW, outH, false), nil
}

// ReflectionPad extends an image by mirroring its edge pixels outwards. It is exported for the drivers that pad to a
// model's alignment requirement rather than to a fixed tile size.
func ReflectionPad(img image.Image, left, top, right, bottom int) image.Image {
	bounds := img.Bounds()
	width, height := bounds.Dx(), bounds.Dy()

	paddedWidth := width + left + right
	paddedHeight := height + top + bottom
	padded := image.NewRGBA(image.Rect(0, 0, paddedWidth, paddedHeight))

	// Mirrors idx back into [0, length). A pad wider than the source has no true mirror to draw from - one reflection
	// is not enough to get back inside - so the result is clamped to the nearest edge pixel.
	//
	// That case is reachable: a tile only a few pixels wide is padded out to the full tile size, so padRight exceeds
	// the tile's own width. The previous per-pixel version read those indices out of bounds through At(), which
	// returns transparent black, and so wrote a hard black border into the very edge the padding exists to make
	// continuous. Repeating the edge pixel is both correct-in-spirit and what every other reflect-pad implementation
	// falls back to.
	reflectIndex := func(idx, length int) int {
		if idx < 0 {
			idx = -idx - 1
		} else if idx >= length {
			idx = 2*length - idx - 1
		}

		return min(length-1, max(0, idx))
	}

	// Copy original image to center
	draw.Draw(padded, image.Rect(left, top, left+width, top+height), img, bounds.Min, draw.Src)

	// Everything below mirrors within padded rather than sampling img again. The centre already holds exactly the
	// pixels draw.Draw converted, so copying bytes out of it is identical to re-reading the source through At()/Set()
	// - and it drops the per-pixel interface dispatch and color boxing that the rest of this file is careful to avoid.
	// This matters beyond the edge tiles: the diffusion upscaler pads the whole image through here.
	stride := padded.Stride
	pix := padded.Pix

	// Pad left and right edges, across the rows the source occupies.
	for y := range height {
		row := (y + top) * stride

		for x := range left {
			dst := row + x*4
			src := row + (left+reflectIndex(left-x-1, width))*4
			copy(pix[dst:dst+4], pix[src:src+4])
		}

		for x := range right {
			dst := row + (left+width+x)*4
			src := row + (left+reflectIndex(width+x, width))*4
			copy(pix[dst:dst+4], pix[src:src+4])
		}
	}

	// Pad top and bottom edges. The corners come along for free: with the left and right padding already written, each
	// of these is a whole-row copy rather than a per-pixel walk.
	rowBytes := paddedWidth * 4

	for y := range top {
		dst := y * stride
		src := (reflectIndex(top-y-1, height) + top) * stride
		copy(pix[dst:dst+rowBytes], pix[src:src+rowBytes])
	}

	for y := range bottom {
		dst := (height + top + y) * stride
		src := (reflectIndex(height+y, height) + top) * stride
		copy(pix[dst:dst+rowBytes], pix[src:src+rowBytes])
	}

	return padded
}
