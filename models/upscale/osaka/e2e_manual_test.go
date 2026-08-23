//go:build manual

package osaka

import (
	"context"
	"image"
	"image/jpeg"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/types"
)

// TestOsakaFullPipeline runs the whole thing the way the app will: resample to the target size, restore it region by
// region, blend, colour-correct, and write the result out to look at.
func TestOsakaFullPipeline(t *testing.T) {
	bootstrap(t)

	const scale = 2.0

	// Auto, not CPU: the app resolves the provider this way, and pinning this to CPU is what let a CoreML abort
	// reach a user despite the pipeline "passing" end to end.
	//
	// Built through New rather than by hand, so the role-to-session binding under test is the one the app uses.
	m, err := New(context.Background(), Op(scale, types.PrecisionFp16), types.ExecutionProviderAuto, nil)
	if err != nil {
		t.Fatalf("load model: %v", err)
	}
	defer m.Destroy()

	f, err := os.Open(samplePath)
	if err != nil {
		t.Fatalf("open sample: %v", err)
	}
	src, err := jpeg.Decode(f)
	f.Close()
	if err != nil {
		t.Fatalf("decode sample: %v", err)
	}

	outDir := os.TempDir()

	lastPct := -1.0
	progress := func(p float64) {
		if p-lastPct >= 0.2 {
			t.Logf("   progress %.0f%%", p*100)
			lastPct = p
		}
	}

	start := time.Now()
	out, err := runPipeline(context.Background(), m, src, scale, progress)
	elapsed := time.Since(start)

	if err != nil {
		t.Fatalf("pipeline: %v", err)
	}

	b := src.Bounds()
	ob := out.Bounds()
	t.Logf("%dx%d -> %dx%d in %v", b.Dx(), b.Dy(), ob.Dx(), ob.Dy(), elapsed.Round(time.Second))

	if ob.Dx() != int(float64(b.Dx())*scale) || ob.Dy() != int(float64(b.Dy())*scale) {
		t.Fatalf("wrong output size: got %dx%d", ob.Dx(), ob.Dy())
	}

	// A lanczos-only version at the same size, as the thing to compare against by eye.
	ref := imaging.Resize(src, ob.Dx(), ob.Dy(), imaging.Lanczos)

	for name, img := range map[string]image.Image{"osaka": out, "lanczos": ref} {
		path := filepath.Join(outDir, "osaka_e2e_"+name+".png")
		if err := imaging.Save(img, path); err != nil {
			t.Logf("could not save %s: %v", name, err)
			continue
		}
		t.Logf("wrote %s", path)
	}
}
