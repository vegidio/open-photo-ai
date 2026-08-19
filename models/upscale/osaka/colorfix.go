package osaka

// defaultColorFixLevels is how many à trous levels separate "colour and illumination" from "detail". Five levels
// reach a support of roughly 60 pixels, which is well above the texture SeedVR2 synthesizes and well below the scale
// of the tonal drift being corrected.
const defaultColorFixLevels = 5

// waveletColorFix replaces the low-frequency content of result with that of reference, keeping result's high
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

	for c := range 3 {
		lo, hi := c*plane, (c+1)*plane

		resultLow := lowFrequency(result[lo:hi], width, height, levels)
		referenceLow := lowFrequency(reference[lo:hi], width, height, levels)

		for i := range plane {
			result[lo+i] += referenceLow[i] - resultLow[i]
		}
	}

	return result
}

// lowFrequency extracts the low-frequency component of one channel plane by repeatedly convolving with a B3-spline
// kernel whose taps are spread further apart at each level.
//
// This is the à trous ("with holes") transform: the kernel is never subsampled, so every level stays at full
// resolution and the result aligns with the input pixel for pixel. That alignment is the point - a decimated wavelet
// would need interpolation back up, which reintroduces exactly the low-frequency error being measured.
func lowFrequency(plane []float32, width, height, levels int) []float32 {
	current := make([]float32, len(plane))
	copy(current, plane)

	scratch := make([]float32, len(plane))

	for level := range levels {
		dilation := 1 << level

		convolveHorizontal(current, scratch, width, height, dilation)
		convolveVertical(scratch, current, width, height, dilation)
	}

	return current
}

// b3Spline is [1, 4, 6, 4, 1] / 16, the standard à trous scaling function.
var b3Spline = [5]float32{1.0 / 16, 4.0 / 16, 6.0 / 16, 4.0 / 16, 1.0 / 16}

func convolveHorizontal(src, dst []float32, width, height, dilation int) {
	for y := range height {
		row := y * width

		for x := range width {
			var sum float32

			for k := range 5 {
				sum += b3Spline[k] * src[row+clampIndex(x+(k-2)*dilation, width)]
			}

			dst[row+x] = sum
		}
	}
}

func convolveVertical(src, dst []float32, width, height, dilation int) {
	for y := range height {
		for x := range width {
			var sum float32

			for k := range 5 {
				sum += b3Spline[k] * src[clampIndex(y+(k-2)*dilation, height)*width+x]
			}

			dst[y*width+x] = sum
		}
	}
}

// clampIndex replicates the edge pixel outside the image, which keeps the transform from darkening the border the
// way zero-padding would.
func clampIndex(i, n int) int {
	if i < 0 {
		return 0
	}
	if i >= n {
		return n - 1
	}

	return i
}
