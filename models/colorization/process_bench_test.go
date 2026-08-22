package colorization

import "testing"

// benchW, benchH is a 12 MP photo — the size at which the compose path's cost actually matters.
const benchW, benchH = 4000, 3000

// BenchmarkCompose and BenchmarkComposeReference exist as a pair: the reference is the pre-fusion implementation, so
// running both shows what fusing the passes and replacing the per-pixel math.Pow calls with lookup tables bought.
// Measured on an M2 Max: 2812 ms and 192 MB down to 231 ms and 48 MB, the latter being just the output image.
func BenchmarkCompose(b *testing.B) {
	img := synth(benchW, benchH)
	aPlane, bPlane := synthChroma(inputSize)

	b.ResetTimer()
	b.ReportAllocs()

	for range b.N {
		compose(img, aPlane, bPlane, inputSize)
	}
}

func BenchmarkComposeReference(b *testing.B) {
	img := synth(benchW, benchH)
	aPlane, bPlane := synthChroma(inputSize)

	b.ResetTimer()
	b.ReportAllocs()

	for range b.N {
		composeReference(img, aPlane, bPlane, inputSize)
	}
}
