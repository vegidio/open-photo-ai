package colorbalance

import (
	"image"
	"testing"
)

// benchW, benchH is a 12 MP photo - the size at which the blend's cost actually matters.
const benchW, benchH = 4000, 3000

// benchCanvas mirrors what the saopaulo variant ships: a 656 square, here with a landscape crop inside it.
const benchCanvas, benchCropW, benchCropH = 656, 656, 492

// BenchmarkBlendWeighted and BenchmarkBlendWeightedReference exist as a pair: the reference is the pre-fusion
// implementation, so running both shows what fusing the passes bought. The reference materialises a full-resolution
// float buffer per rendering and one per upsampled weight plane, which at this size is where its allocations go.
func BenchmarkBlendWeighted(b *testing.B) {
	img := synth(benchW, benchH, image.Point{}, false, false)
	maps := synthMaps()
	weights := synthWeights(len(maps)+1, benchCanvas, benchCropW, benchCropH)

	b.ResetTimer()
	b.ReportAllocs()

	for range b.N {
		blendWeighted(img, maps, weights, benchCanvas, benchCropW, benchCropH)
	}
}

func BenchmarkBlendWeightedReference(b *testing.B) {
	img := synth(benchW, benchH, image.Point{}, false, false)
	maps := synthMaps()
	weights := synthWeights(len(maps)+1, benchCanvas, benchCropW, benchCropH)

	b.ResetTimer()
	b.ReportAllocs()

	for range b.N {
		referenceBlend(img, maps, weights, benchCanvas, benchCropW, benchCropH)
	}
}

// BenchmarkFitPolynomialMappings covers the other half of ProcessMixed's CPU work: the shared normal-equation
// accumulation over the canvas, which is what fitting two renderings at once is meant to halve.
func BenchmarkFitPolynomialMappings(b *testing.B) {
	src := randomSamples(benchCanvas*benchCropH, 1)
	s := randomSamples(len(src), 2)
	t := randomSamples(len(src), 3)

	b.Run("together", func(b *testing.B) {
		b.ReportAllocs()
		for range b.N {
			if _, err := fitPolynomialMappings(src, s, t); err != nil {
				b.Fatal(err)
			}
		}
	})

	b.Run("separately", func(b *testing.B) {
		b.ReportAllocs()
		for range b.N {
			if _, err := fitPolynomialMapping(src, s); err != nil {
				b.Fatal(err)
			}
			if _, err := fitPolynomialMapping(src, t); err != nil {
				b.Fatal(err)
			}
		}
	})
}
