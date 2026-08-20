package osaka

import "github.com/vegidio/open-photo-ai/internal/utils"

// defaultColorFixLevels is how many à trous levels separate "colour and illumination" from "detail". Five levels
// reach a support of roughly 60 pixels, which is well above the texture SeedVR2 synthesizes and well below the scale
// of the tonal drift being corrected.
const defaultColorFixLevels = 5

// waveletColorFix replaces the low-frequency content of the result with that of reference, keeping result's high
// frequencies. Both are planar CHW float32 of the same dimensions; result is modified in place and returned.
//
// A single diffusion step drifts in overall colour and brightness. Left uncorrected the output carries a cast, and -
// because each tile drifts independently - the drift reads as seams that no amount of blending removes, since the
// tiles genuinely disagree about the colour of the region they share.
//
// Two things about how this is applied matter as much as the transform itself:
//
//   - It runs once on the assembled image, never per tile. Fixing each tile against its own reference crop bakes the
//     per-tile drift in as a step change at the boundary instead of removing it.
//   - The reference is the resampled image the model was conditioned on, not the original input, so the correction
//     only undoes what the model changed rather than also undoing the resampling.
func waveletColorFix(result, reference []float32, width, height, levels int) []float32 {
	if levels <= 0 || len(result) != len(reference) {
		return result
	}

	plane := width * height

	// Three planes, allocated once for all three channels rather than two per lowFrequency call. At a 4x 12 MP output
	// a plane is ~770 MB, so the twelve short-lived allocations this replaces were pure churn on a machine already
	// holding a 7 GB model.
	resultLow := make([]float32, plane)
	referenceLow := make([]float32, plane)
	scratch := make([]float32, plane)

	for c := range 3 {
		lo, hi := c*plane, (c+1)*plane

		lowFrequency(resultLow, result[lo:hi], scratch, width, height, levels)
		lowFrequency(referenceLow, reference[lo:hi], scratch, width, height, levels)

		for i := range plane {
			result[lo+i] += referenceLow[i] - resultLow[i]
		}
	}

	return result
}

// lowFrequency writes the low-frequency component of plane into dst by repeatedly convolving with a B3-spline kernel
// whose taps are spread further apart at each level. scratch is working space of the same length; both it and dst are
// overwritten.
//
// This is the transform "with holes": the kernel is never subsampled, so every level stays at full resolution and the
// result aligns with the input pixel for pixel. That alignment is the point - a decimated wavelet would need
// interpolation back up, which reintroduces exactly the low-frequency error being measured.
func lowFrequency(dst, plane, scratch []float32, width, height, levels int) {
	copy(dst, plane)

	for level := range levels {
		dilation := 1 << level

		convolveHorizontal(dst, scratch, width, height, dilation)
		convolveVertical(scratch, dst, width, height, dilation)
	}
}

// b3Spline is [1, 4, 6, 4, 1] / 16, the standard à trous scaling function.
var b3Spline = [5]float32{1.0 / 16, 4.0 / 16, 6.0 / 16, 4.0 / 16, 1.0 / 16}

// The two convolutions below are deliberately not folded into one axis-generic function. Each hoists its clamping out
// of the inner loop in the way its own axis allows, and those ways differ: along a row the clamp depends on the pixel,
// so the row is split into a clamped margin and an unclamped interior; down a column the five row offsets are the same
// for every pixel in the row, so they are computed once. A shared implementation would have to either keep the clamp
// per tap - the cost being removed here - or iterate columns outermost, which strides across memory instead of along
// it. Colour fix runs over the whole assembled image and is the one CPU-bound pass that is not ONNX, so this is where
// the duplication earns its keep.
//
// Both keep the original left-to-right summation order, so the result is bit-identical to the naive form.

// convolveHorizontal filters along each row. Only pixels within 2*dilation of an edge can clamp - at most 32 columns
// at the deepest level - so the interior runs branch-free.
func convolveHorizontal(src, dst []float32, width, height, dilation int) {
	margin := min(2*dilation, width)
	interiorEnd := max(width-2*dilation, margin)

	for y := range height {
		row := src[y*width : (y+1)*width]
		out := dst[y*width : (y+1)*width]

		for x := range margin {
			out[x] = clampedRowTap(row, x, dilation, width)
		}

		for x := margin; x < interiorEnd; x++ {
			out[x] = b3Spline[0]*row[x-2*dilation] +
				b3Spline[1]*row[x-dilation] +
				b3Spline[2]*row[x] +
				b3Spline[3]*row[x+dilation] +
				b3Spline[4]*row[x+2*dilation]
		}

		for x := interiorEnd; x < width; x++ {
			out[x] = clampedRowTap(row, x, dilation, width)
		}
	}
}

// clampedRowTap is the edge case of convolveHorizontal: replicating the edge pixel outside the image, which keeps the
// transform from darkening the border the way zero-padding would.
func clampedRowTap(row []float32, x, dilation, width int) float32 {
	var sum float32
	for k := range 5 {
		sum += b3Spline[k] * row[utils.ClampInt(x+(k-2)*dilation, 0, width-1)]
	}

	return sum
}

// convolveVertical filters down each column. The five source rows a given output row reads are the same for every
// pixel in it, so they are clamped once per row rather than once per pixel.
func convolveVertical(src, dst []float32, width, height, dilation int) {
	for y := range height {
		var rows [5]int
		for k := range 5 {
			rows[k] = utils.ClampInt(y+(k-2)*dilation, 0, height-1) * width
		}

		out := dst[y*width : (y+1)*width]

		for x := range width {
			out[x] = b3Spline[0]*src[rows[0]+x] +
				b3Spline[1]*src[rows[1]+x] +
				b3Spline[2]*src[rows[2]+x] +
				b3Spline[3]*src[rows[3]+x] +
				b3Spline[4]*src[rows[4]+x]
		}
	}
}
