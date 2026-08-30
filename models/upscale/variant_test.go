package upscale

import (
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// TestGraphModelIds pins how a diffusion variant's graphs resolve to artifact names, which is the one place a wrong
// string is neither a compile error nor a test failure elsewhere: deps.ModelDependency finds no manifest entry for it,
// downloads it unverified, and the 404 surfaces as a session that would not open.
//
// The case that matters is the mixed one. Osaka publishes its diffusion transformer both as fp16 and quantized to
// int8, but only one pair of fp16 VAE halves, shared by both builds. So an int8 operation must name an int8 DiT and
// fp16 VAEs — not the `_int8` VAE names, which do not exist.
func TestGraphModelIds(t *testing.T) {
	graphs := []GraphSpec{
		{Role: "dit", Suffix: ""},
		{Role: "encoder", Suffix: "_vae_encoder", Precision: types.PrecisionFp16},
		{Role: "decoder", Suffix: "_vae_decoder", Precision: types.PrecisionFp16},
	}

	tests := []struct {
		precision types.Precision
		want      []string
	}{
		{types.PrecisionFp16, []string{"up_osaka_fp16", "up_osaka_vae_encoder_fp16", "up_osaka_vae_decoder_fp16"}},
		{types.PrecisionInt8, []string{"up_osaka_int8", "up_osaka_vae_encoder_fp16", "up_osaka_vae_decoder_fp16"}},
	}

	for _, tt := range tests {
		t.Run(string(tt.precision), func(t *testing.T) {
			for i, g := range graphs {
				if got := g.modelId("osaka", tt.precision); got != tt.want[i] {
					t.Errorf("%s: got %q, want %q", g.Role, got, tt.want[i])
				}
			}
		})
	}
}

// A graph with no pinned precision must follow the operation, whatever it is - that is what keeps the unpinned case
// from quietly acquiring a default.
func TestGraphModelIdFollowsTheOperation(t *testing.T) {
	g := GraphSpec{Role: "dit"}

	for _, p := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16, types.PrecisionInt8} {
		if got, want := g.modelId("osaka", p), "up_osaka_"+string(p); got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	}
}
