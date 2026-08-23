//go:build manual

package osaka

import (
	"context"
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// TestOsakaRestoresDegradedImage runs the real pipeline path on a softened image and checks that the model actually
// improves it. This is the end-to-end claim the model is here to make, so it is asserted rather than merely printed.
func TestOsakaRestoresDegradedImage(t *testing.T) {
	bootstrap(t)

	// CPU, deliberately: this test asserts on the numbers the model produces, and CoreML is excluded for this
	// pipeline precisely because it returns plausible-looking wrong ones.
	m, err := New(context.Background(), Op(1, types.PrecisionFp16), types.ExecutionProviderCPU, nil)
	if err != nil {
		t.Fatalf("load model: %v", err)
	}
	defer m.Destroy()

	pristine, degraded := sweepImages(t)
	basePixels := utils.ImageToCHW(degraded, false, true)
	truePixels := utils.ImageToCHW(pristine, false, true)

	baseline := psnrCHW(basePixels, truePixels)
	baseDetail := detailEnergy(basePixels, sweepEdge, sweepEdge)
	trueDetail := detailEnergy(truePixels, sweepEdge, sweepEdge)

	t.Logf("input  : psnr(true)=%.2f dB  detail=%.4f", baseline, baseDetail)
	t.Logf("pristine:                    detail=%.4f", trueDetail)

	out, err := restoreRegion(context.Background(), m, basePixels, sweepEdge, sweepEdge, 0, 0)
	if err != nil {
		t.Fatalf("restore: %v", err)
	}

	got := psnrCHW(out, truePixels)
	gotDetail := detailEnergy(out, sweepEdge, sweepEdge)

	t.Logf("restored: psnr(true)=%.2f dB  detail=%.4f  psnr(input)=%.2f dB",
		got, gotDetail, psnrCHW(out, basePixels))
	t.Logf("detail recovered: %.1f%% of the gap between input and pristine",
		(gotDetail-baseDetail)/(trueDetail-baseDetail)*100)

	// PSNR is the wrong thing to maximize here and must not be asserted upwards. A generative restorer synthesizes
	// texture that is plausible but not pixel-aligned with the original, which PSNR punishes - a blurry image always
	// scores well on it. What PSNR is good for is catching divergence: the output must stay recognisably the same
	// picture rather than becoming a different one.
	if got < baseline-2 {
		t.Errorf("the output diverged from the input: %.2f dB vs %.2f dB", got, baseline)
	}

	// The claim worth asserting is that detail was actually restored, and restored towards the original rather than
	// past it. Runaway detail is the signature of a broken conditioning - the failed convention sweep produced four
	// to twenty times the input's detail while scoring 15 dB.
	recovered := (gotDetail - baseDetail) / (trueDetail - baseDetail)
	if recovered < 0.3 {
		t.Errorf("only %.0f%% of the missing detail was restored", recovered*100)
	}
	if recovered > 1.5 {
		t.Errorf("detail overshot the original by %.0f%%, which means noise rather than restoration", recovered*100)
	}
}
