package utils

// The operation ID is the contract between the frontend (src/operations/factory.ts, which builds these strings) and
// this package, and it feeds both the model registry key and the on-disk image cache key. These tests pin that
// contract: every ID form the frontend can emit, the legacy form still in old cache entries, and the inputs that ride
// alongside the identity rather than inside it.

import (
	"fmt"
	guitypes "gui/types"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// Mirrors internal/utils.ParamIntensity and IntensityCacheKey, which the gui module can't import.
const paramIntensity = "intensity"

func intensityCacheKey(i float32) string { return fmt.Sprintf("i=%.3g", i) }

func intensityFromParams(params map[string]any) float32 {
	if v, ok := params[paramIntensity].(float32); ok {
		return v
	}
	return 1.0
}

// Id() deliberately omits the intensity so the model registry reuses one session across intensities; the intensity
// rides in Params()/CacheKey() instead. Scale, by contrast, is part of the identity because it selects the model.
func parseOne(t *testing.T, id string) types.Operation {
	t.Helper()
	ops, err := IdsToOperations([]string{id}, guitypes.InferenceParams{})
	if err != nil {
		t.Fatalf("%s: %v", id, err)
	}
	if len(ops) != 1 {
		t.Fatalf("%s: got %d operations, want 1", id, len(ops))
	}
	return ops[0]
}

func TestIdsToOperationsIdentity(t *testing.T) {
	cases := map[string]string{
		// intensity ops: identity drops the intensity segment
		"dn_stockholm_1_fp32":   "dn_stockholm_fp32",
		"dn_malmo_0.5_fp32":     "dn_malmo_fp32",
		"dn_gothenburg_0_fp16":  "dn_gothenburg_fp16",
		"sh_moscow_1_fp32":      "sh_moscow_fp32",
		"sh_novgorod_0.25_fp32": "sh_novgorod_fp32",
		"sh_petersburg_2_fp16":  "sh_petersburg_fp16",
		"la_paris_0.5_fp32":     "la_paris_fp32",
		"cb_rio_0.5_fp32":       "cb_rio_fp32",
		// legacy 3-segment denoise/sharpen form still parses
		"dn_stockholm_fp32": "dn_stockholm_fp32",
		"sh_moscow_fp16":    "sh_moscow_fp16",
		// scale is part of the identity
		"up_tokyo_4x_fp32":   "up_tokyo_4x_fp32",
		"up_kyoto_2x_fp32":   "up_kyoto_2x_fp32",
		"up_saitama_1x_fp16": "up_saitama_1x_fp16",
		// face recovery
		"fr_athens_fp32":    "fr_athens_fp32",
		"fr_santorini_fp16": "fr_santorini_fp16",
	}

	for id, wantIdentity := range cases {
		if got := parseOne(t, id).Id(); got != wantIdentity {
			t.Errorf("%q: Id() = %q, want %q", id, got, wantIdentity)
		}
	}
}

func TestIdsToOperationsCarriesIntensity(t *testing.T) {
	cases := map[string]float32{
		"dn_stockholm_0.25_fp32": 0.25,
		"dn_malmo_1_fp32":        1,
		"sh_petersburg_0_fp32":   0,
		"la_paris_0.5_fp32":      0.5,
		"cb_rio_0.75_fp32":       0.75,
		// legacy form defaults to 1.0
		"dn_stockholm_fp32": 1.0,
		"sh_moscow_fp32":    1.0,
	}

	for id, want := range cases {
		op := parseOne(t, id)

		p, ok := op.(types.Parameterized)
		if !ok {
			t.Fatalf("%q: operation is not Parameterized", id)
		}
		if got := intensityFromParams(p.Params()); got != want {
			t.Errorf("%q: intensity = %v, want %v", id, got, want)
		}

		// Different intensities must not collide in the image cache.
		ck, ok := op.(types.CacheKeyer)
		if !ok {
			t.Fatalf("%q: operation is not a CacheKeyer", id)
		}
		if ck.CacheKey() != intensityCacheKey(want) {
			t.Errorf("%q: CacheKey() = %q, want %q", id, ck.CacheKey(), intensityCacheKey(want))
		}
	}
}

func TestIdsToOperationsPreservesOrder(t *testing.T) {
	ids := []string{"dn_stockholm_1_fp32", "fr_athens_fp32", "up_kyoto_2x_fp32"}
	want := []string{"dn_stockholm_fp32", "fr_athens_fp32", "up_kyoto_2x_fp32"}

	ops, err := IdsToOperations(ids, guitypes.InferenceParams{})
	if err != nil {
		t.Fatal(err)
	}
	for i, op := range ops {
		if op.Id() != want[i] {
			t.Errorf("position %d: got %q, want %q", i, op.Id(), want[i])
		}
	}
}

func TestIdsToOperationsRejectsBadInput(t *testing.T) {
	bad := []string{
		"dn_stockholm",         // too few segments
		"nope",                 // too few segments
		"xx_unknown_1_fp32",    // unknown model
		"la_paris_fp32",        // paris requires an intensity
		"cb_rio_fp32",          // rio requires an intensity
		"up_tokyo_fp32",        // upscale requires a scale
		"dn_stockholm_ab_fp32", // unparseable intensity
		"up_tokyo_abx_fp32",    // unparseable scale
	}

	for _, id := range bad {
		if _, err := IdsToOperations([]string{id}, guitypes.InferenceParams{}); err == nil {
			t.Errorf("%q: expected an error, got nil", id)
		}
	}
}
