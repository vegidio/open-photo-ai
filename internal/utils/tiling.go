package utils

import (
	"context"
	"image"
	"image/draw"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
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
	columns := len(grid.offsets(width))

	step := 1 / float64(len(tiles))
	total := 0.0

	for i, rect := range tiles {
		if err := ctx.Err(); err != nil {
			return nil, errors.Wrap(err, "context cancelled")
		}

		tileX, tileY := rect.Min.X, rect.Min.Y
		tileW, tileH := rect.Dx(), rect.Dy()

		paddedTile := prepareTileForInference(img, tileX, tileY, tileW, tileH, defaultTileSize)

		processedTile, err := processTile(session, paddedTile, tileW, tileH, scale, cfg.divergenceThreshold)
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

// Tiles returns the tile rectangles in row-major order. An invalid geometry yields a single tile covering the whole
// image, so a caller can never be handed an empty partitioning.
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

// processTile runs the model and crops the padding back off. The crop bounds are scaled by scale to match the
// inference output dimensions (scale is 1 for denoise).
func processTile(session *Session, tile image.Image, tileW, tileH, scale int, divergenceThreshold float32) (image.Image, error) {
	processedTile, err := runTileInference(session, tile, scale, divergenceThreshold)
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
func runTileInference(session *Session, tile image.Image, scale int, divergenceThreshold float32) (image.Image, error) {
	bounds := tile.Bounds()
	h, w := bounds.Dy(), bounds.Dx()

	inputData := ImageToCHW(tile, true, false)

	inputShape := ort.NewShape(1, 3, int64(h), int64(w))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	outputShape := ort.NewShape(1, 3, int64(h*scale), int64(w*scale))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create output tensor")
	}
	defer outputTensor.Destroy()

	err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	outputData := outputTensor.GetData()
	outW := int(outputShape[3])
	outH := int(outputShape[2])

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

	reflectIndex := func(idx, max int) int {
		if idx < 0 {
			return -idx - 1
		}
		if idx >= max {
			return 2*max - idx - 1
		}
		return idx
	}

	// Copy original image to center
	draw.Draw(padded, image.Rect(left, top, left+width, top+height), img, bounds.Min, draw.Src)

	// Pad left and right edges
	for y := range height {
		srcY := bounds.Min.Y + y
		dstY := y + top

		for x := range left {
			srcX := bounds.Min.X + reflectIndex(left-x-1, width)
			padded.Set(x, dstY, img.At(srcX, srcY))
		}

		for x := range right {
			srcX := bounds.Min.X + reflectIndex(width+x, width)
			padded.Set(width+left+x, dstY, img.At(srcX, srcY))
		}
	}

	// Pad top and bottom edges (including corners)
	for x := range paddedWidth {
		for y := range top {
			srcY := reflectIndex(top-y-1, height) + top
			padded.Set(x, y, padded.At(x, srcY))
		}

		for y := range bottom {
			srcY := reflectIndex(height+y, height) + top
			padded.Set(x, height+top+y, padded.At(x, srcY))
		}
	}

	return padded
}
