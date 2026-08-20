//go:build manual

// Support for the tests that need the real 6.8 GB model, which is why they are behind a build tag rather than in the
// default suite:
//
//	go test -tags manual -run TestOsaka -v -timeout 60m ./models/upscale/osaka/
package osaka

import (
	"image"
	"image/jpeg"
	"math"
	"os"
	"path/filepath"
	"testing"

	"github.com/disintegration/imaging"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

const (
	samplePath = "../../../cmd/perf/test.dat" // a 640x640 JPEG
	sweepEdge  = ditRegionEdge                // the only region size the DiT accepts
	degradeBy  = 4                            // how much the input is softened before restoration
)

// bootstrap brings up ONNX Runtime without going through the root package, which imports this one.
func bootstrap(t *testing.T) {
	t.Helper()

	internal.AppName = "open-photo-ai"

	modelData, err := utils.LoadModelData()
	if err != nil {
		t.Fatalf("load model manifest: %v", err)
	}
	internal.ModelData = modelData

	dir, err := fs.MkUserConfigDir(internal.AppName, internal.RuntimeDir)
	if err != nil {
		t.Fatalf("runtime dir: %v", err)
	}

	pinned, found := internal.PinnedArchive("onnx")
	if !found {
		t.Fatalf("no ONNX Runtime is published for this platform")
	}

	ort.SetSharedLibraryPath(filepath.Join(dir, pinned.Lib))
	if err = ort.InitializeEnvironment(); err != nil {
		t.Fatalf("initialize ONNX Runtime: %v", err)
	}

	t.Cleanup(func() { _ = ort.DestroyEnvironment() })
}

// sweepImages returns the pristine crop and the softened version handed to the model. Restoring a degraded input is
// what the model is for, so it discriminates far better than a clean one: a configuration that merely passes its
// input through scores well against the input and badly against the pristine original.
func sweepImages(t *testing.T) (pristine, degraded image.Image) {
	t.Helper()

	f, err := os.Open(samplePath)
	if err != nil {
		t.Fatalf("open sample: %v", err)
	}
	defer f.Close()

	src, err := jpeg.Decode(f)
	if err != nil {
		t.Fatalf("decode sample: %v", err)
	}

	// The sample is smaller than a region, so it is scaled up to one rather than cropped - CropCenter would
	// silently return something smaller and restoreRegion only accepts an exact region.
	pristine = imaging.Resize(imaging.CropCenter(src, min(src.Bounds().Dx(), src.Bounds().Dy()),
		min(src.Bounds().Dx(), src.Bounds().Dy())), sweepEdge, sweepEdge, imaging.Lanczos)
	small := imaging.Resize(pristine, sweepEdge/degradeBy, sweepEdge/degradeBy, imaging.Lanczos)
	degraded = imaging.Resize(small, sweepEdge, sweepEdge, imaging.Lanczos)

	return pristine, degraded
}

func psnrCHW(a, b []float32) float64 {
	var mse float64
	for i := range a {
		d := float64(a[i] - b[i])
		mse += d * d
	}

	mse /= float64(len(a))
	if mse <= 0 {
		return math.Inf(1)
	}

	// The data is in [-1,1], so the peak-to-peak range is 2.
	return 10 * math.Log10(4/mse)
}

// detailEnergy is the mean absolute Laplacian: it responds to edges and texture and ignores the smooth content that
// PSNR is dominated by.
func detailEnergy(data []float32, width, height int) float64 {
	plane := width * height
	var sum float64

	for c := range 3 {
		for y := 1; y < height-1; y++ {
			for x := 1; x < width-1; x++ {
				i := c*plane + y*width + x
				lap := 4*data[i] - data[i-1] - data[i+1] - data[i-width] - data[i+width]
				sum += math.Abs(float64(lap))
			}
		}
	}

	return sum / float64(3*plane)
}
