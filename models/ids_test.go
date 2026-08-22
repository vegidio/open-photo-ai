package models_test

import (
	"testing"

	"github.com/vegidio/open-photo-ai/models/colorization/delhi"
	"github.com/vegidio/open-photo-ai/models/colorization/jaipur"
	"github.com/vegidio/open-photo-ai/models/colorization/mumbai"
	"github.com/vegidio/open-photo-ai/models/denoise/gothenburg"
	"github.com/vegidio/open-photo-ai/models/denoise/malmo"
	"github.com/vegidio/open-photo-ai/models/denoise/stockholm"
	"github.com/vegidio/open-photo-ai/models/sharpen/moscow"
	"github.com/vegidio/open-photo-ai/models/sharpen/novgorod"
	"github.com/vegidio/open-photo-ai/models/sharpen/petersburg"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
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

	var up types.Operation = kyoto.Op(2, types.PrecisionFp32)
	if _, ok := up.(types.Parameterized); ok {
		t.Error("upscale Op unexpectedly implements types.Parameterized")
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
