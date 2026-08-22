package models_test

import (
	"testing"

	"github.com/vegidio/open-photo-ai/models/colorbalance/rio"
	"github.com/vegidio/open-photo-ai/models/colorization/delhi"
	"github.com/vegidio/open-photo-ai/models/colorization/jaipur"
	"github.com/vegidio/open-photo-ai/models/colorization/mumbai"
	"github.com/vegidio/open-photo-ai/models/denoise/gothenburg"
	"github.com/vegidio/open-photo-ai/models/denoise/malmo"
	"github.com/vegidio/open-photo-ai/models/denoise/stockholm"
	"github.com/vegidio/open-photo-ai/models/detection/newyork"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/sharpen/moscow"
	"github.com/vegidio/open-photo-ai/models/sharpen/novgorod"
	"github.com/vegidio/open-photo-ai/models/sharpen/petersburg"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/osaka"
	"github.com/vegidio/open-photo-ai/models/upscale/saitama"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

// Operation IDs are not an internal detail: they key the on-disk image cache, the GUI's operation table, and the
// downloaded model artifacts. A change here silently invalidates every user's cache and can break model resolution,
// so the expected strings below are pinned literals rather than anything derived from the code under test.

func TestIntensityOperationIds(t *testing.T) {
	builders := map[string]func(float32, types.Precision) types.Operation{
		"stockholm":  func(i float32, p types.Precision) types.Operation { return stockholm.Op(i, p) },
		"malmo":      func(i float32, p types.Precision) types.Operation { return malmo.Op(i, p) },
		"gothenburg": func(i float32, p types.Precision) types.Operation { return gothenburg.Op(i, p) },
		"moscow":     func(i float32, p types.Precision) types.Operation { return moscow.Op(i, p) },
		"novgorod":   func(i float32, p types.Precision) types.Operation { return novgorod.Op(i, p) },
		"petersburg": func(i float32, p types.Precision) types.Operation { return petersburg.Op(i, p) },
		"paris":      func(i float32, p types.Precision) types.Operation { return paris.Op(i, p) },
		"rio":        func(i float32, p types.Precision) types.Operation { return rio.Op(i, p) },
	}

	tests := []struct{ variant, precision, want string }{
		{"stockholm", "fp32", "dn_stockholm_fp32"},
		{"stockholm", "fp16", "dn_stockholm_fp16"},
		{"malmo", "fp32", "dn_malmo_fp32"},
		{"malmo", "fp16", "dn_malmo_fp16"},
		{"gothenburg", "fp32", "dn_gothenburg_fp32"},
		{"gothenburg", "fp16", "dn_gothenburg_fp16"},
		{"moscow", "fp32", "sh_moscow_fp32"},
		{"moscow", "fp16", "sh_moscow_fp16"},
		{"novgorod", "fp32", "sh_novgorod_fp32"},
		{"novgorod", "fp16", "sh_novgorod_fp16"},
		{"petersburg", "fp32", "sh_petersburg_fp32"},
		{"petersburg", "fp16", "sh_petersburg_fp16"},
		{"paris", "fp32", "la_paris_fp32"},
		{"paris", "fp16", "la_paris_fp16"},
		{"rio", "fp32", "cb_rio_fp32"},
		{"rio", "fp16", "cb_rio_fp16"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			// The intensity is a per-run parameter and must never leak into the identity.
			for _, intensity := range []float32{0, 0.5, 1, -1} {
				got := builders[tt.variant](intensity, types.Precision(tt.precision)).Id()
				if got != tt.want {
					t.Fatalf("intensity %v: got %q, want %q", intensity, got, tt.want)
				}
			}
		})
	}
}

func TestUpscaleOperationIds(t *testing.T) {
	builders := map[string]func(float64, types.Precision) types.Operation{
		"kyoto":   func(s float64, p types.Precision) types.Operation { return kyoto.Op(s, p) },
		"saitama": func(s float64, p types.Precision) types.Operation { return saitama.Op(s, p) },
		"tokyo":   func(s float64, p types.Precision) types.Operation { return tokyo.Op(s, p) },
	}

	tests := []struct {
		variant   string
		scale     float64
		precision string
		want      string
	}{
		{"kyoto", 1, "fp32", "up_kyoto_1x_fp32"},
		{"kyoto", 2, "fp32", "up_kyoto_2x_fp32"},
		{"kyoto", 2.5, "fp32", "up_kyoto_2.5x_fp32"},
		{"kyoto", 4, "fp16", "up_kyoto_4x_fp16"},
		{"kyoto", 8, "fp32", "up_kyoto_8x_fp32"},
		{"saitama", 2, "fp32", "up_saitama_2x_fp32"},
		{"saitama", 3.75, "fp16", "up_saitama_3.75x_fp16"},
		{"saitama", 8, "fp16", "up_saitama_8x_fp16"},
		{"tokyo", 2, "fp32", "up_tokyo_2x_fp32"},
		{"tokyo", 4, "fp32", "up_tokyo_4x_fp32"},
		{"tokyo", 8, "fp16", "up_tokyo_8x_fp16"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			got := builders[tt.variant](tt.scale, types.Precision(tt.precision)).Id()
			if got != tt.want {
				t.Fatalf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestColorizationOperationIds(t *testing.T) {
	builders := map[string]func(types.Precision) types.Operation{
		"delhi":  func(p types.Precision) types.Operation { return delhi.Op(p) },
		"mumbai": func(p types.Precision) types.Operation { return mumbai.Op(p) },
		"jaipur": func(p types.Precision) types.Operation { return jaipur.Op(p) },
	}

	tests := []struct{ variant, precision, want string }{
		{"delhi", "fp32", "cl_delhi_fp32"},
		{"delhi", "fp16", "cl_delhi_fp16"},
		{"mumbai", "fp32", "cl_mumbai_fp32"},
		{"mumbai", "fp16", "cl_mumbai_fp16"},
		{"jaipur", "fp32", "cl_jaipur_fp32"},
		{"jaipur", "fp16", "cl_jaipur_fp16"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			got := builders[tt.variant](types.Precision(tt.precision)).Id()
			if got != tt.want {
				t.Fatalf("got %q, want %q", got, tt.want)
			}
		})
	}
}

// TestOperationInterfaces pins the optional interfaces the registry probes for. Losing Parameterized or CacheKeyer on
// the intensity families would make every intensity collide in the image cache.
func TestOperationInterfaces(t *testing.T) {
	var op types.Operation = malmo.Op(0.5, types.PrecisionFp32)

	if _, ok := op.(types.Parameterized); !ok {
		t.Error("denoise Op no longer implements types.Parameterized")
	}
	if _, ok := op.(types.CacheKeyer); !ok {
		t.Error("denoise Op no longer implements types.CacheKeyer")
	}

	// The convolutional upscalers share their Op type with the diffusion upscaler, which does carry a per-run scale,
	// so they satisfy both interfaces statically. What must hold is that they contribute nothing: their scale is
	// already in the Id, and a non-empty CacheKey here would change every existing upscale cache entry's key.
	var up types.Operation = kyoto.Op(2, types.PrecisionFp32)
	if p, ok := up.(types.Parameterized); ok && len(p.Params()) != 0 {
		t.Errorf("convolutional upscale Op contributes params %v, want none", p.Params())
	}
	if ck, ok := up.(types.CacheKeyer); ok && ck.CacheKey() != "" {
		t.Errorf("convolutional upscale Op contributes cache key %q, want \"\"", ck.CacheKey())
	}

	// Osaka is the mirror image: the scale is deliberately absent from its Id, so it must travel in Params and be
	// folded into the cache key, or two scales would collide on one cached image.
	var os types.Operation = osaka.Op(4, types.PrecisionFp16)
	if p, ok := os.(types.Parameterized); !ok || len(p.Params()) == 0 {
		t.Error("osaka Op must carry its scale in Params")
	}
	if ck, ok := os.(types.CacheKeyer); !ok || ck.CacheKey() == "" {
		t.Error("osaka Op must fold its scale into the cache key")
	}

	// Face recovery carries the selected faces per-run; losing either interface would serve a stale recovery.
	var fr types.Operation = athens.Op(types.PrecisionFp32, nil)
	if _, ok := fr.(types.Parameterized); !ok {
		t.Error("face recovery Op no longer implements types.Parameterized")
	}
	if _, ok := fr.(types.CacheKeyer); !ok {
		t.Error("face recovery Op no longer implements types.CacheKeyer")
	}

	// Detection has no per-run inputs; its cache identity must remain the operation Id alone.
	var dt types.Operation = newyork.Op(types.PrecisionFp32)
	if _, ok := dt.(types.Parameterized); ok {
		t.Error("detection Op unexpectedly implements types.Parameterized")
	}
	if _, ok := dt.(types.CacheKeyer); ok {
		t.Error("detection Op unexpectedly implements types.CacheKeyer")
	}

	// Colorization has no per-run inputs; its cache identity must remain the operation Id alone.
	var cl types.Operation = delhi.Op(types.PrecisionFp32)
	if _, ok := cl.(types.Parameterized); ok {
		t.Error("colorization Op unexpectedly implements types.Parameterized")
	}
	if _, ok := cl.(types.CacheKeyer); ok {
		t.Error("colorization Op unexpectedly implements types.CacheKeyer")
	}
}

func TestFaceOperationIds(t *testing.T) {
	builders := map[string]func(types.Precision) types.Operation{
		"athens":    func(p types.Precision) types.Operation { return athens.Op(p, nil) },
		"santorini": func(p types.Precision) types.Operation { return santorini.Op(p, nil) },
		"newyork":   func(p types.Precision) types.Operation { return newyork.Op(p) },
	}

	tests := []struct{ variant, precision, want string }{
		{"athens", "fp32", "fr_athens_fp32"},
		{"athens", "fp16", "fr_athens_fp16"},
		{"santorini", "fp32", "fr_santorini_fp32"},
		{"santorini", "fp16", "fr_santorini_fp16"},
		{"newyork", "fp32", "dt_newyork_fp32"},
		{"newyork", "fp16", "dt_newyork_fp16"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			got := builders[tt.variant](types.Precision(tt.precision)).Id()
			if got != tt.want {
				t.Fatalf("got %q, want %q", got, tt.want)
			}
		})
	}
}

// TestOsakaOperationId pins the two ways Osaka's identity deliberately differs from its siblings': the scale is not in
// the Id (one 7 GB session set serves every scale, so a scale change must not be a registry miss), and the precision
// is pinned to fp16 whatever the caller asks for, because no fp32 build is published.
func TestOsakaOperationId(t *testing.T) {
	for _, scale := range []float64{1, 2, 4, 8} {
		for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
			op := osaka.Op(scale, precision)

			if got := op.Id(); got != "up_osaka_fp16" {
				t.Errorf("scale %v, precision %s: Id() = %q, want %q", scale, precision, got, "up_osaka_fp16")
			}
			if got := op.Precision(); got != types.PrecisionFp16 {
				t.Errorf("scale %v, precision %s: Precision() = %q, want fp16", scale, precision, got)
			}
		}
	}
}
