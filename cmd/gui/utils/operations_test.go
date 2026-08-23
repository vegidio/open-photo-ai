package utils

// The operation ID is the contract between the frontend (src/operations/factory.ts, which builds these strings) and
// this package, and it feeds both the model registry key and the on-disk image cache key. These tests pin that
// contract: every ID form the frontend can emit, the legacy form still in old cache entries, and the inputs that ride
// alongside the identity rather than inside it.

import (
	"fmt"
	"slices"
	"testing"

	guitypes "gui/types"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale"
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
		// ...except for osaka, whose sessions are over 7 GB and are the same whatever the scale, so the scale is a
		// per-run parameter instead and the identity drops it. It also has no fp32 build, so the request is coerced.
		"up_osaka_4x_fp16": "up_osaka_fp16",
		"up_osaka_2x_fp32": "up_osaka_fp16",
		// face recovery
		"fr_athens_fp32":    "fr_athens_fp32",
		"fr_santorini_fp16": "fr_santorini_fp16",
		// colorization has no per-run inputs, so the ID is the identity
		"cl_delhi_fp32":  "cl_delhi_fp32",
		"cl_delhi_fp16":  "cl_delhi_fp16",
		"cl_mumbai_fp32": "cl_mumbai_fp32",
		"cl_mumbai_fp16": "cl_mumbai_fp16",
		"cl_jaipur_fp32": "cl_jaipur_fp32",
		"cl_jaipur_fp16": "cl_jaipur_fp16",
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

// Osaka carries its scale the way the intensity models carry theirs: out of the identity, into Params, and folded
// into the cache key so two scales cannot return each other's cached image.
func TestIdsToOperationsCarriesOsakaScale(t *testing.T) {
	cases := map[string]float64{
		"up_osaka_1x_fp16":   1,
		"up_osaka_2x_fp16":   2,
		"up_osaka_4x_fp16":   4,
		"up_osaka_2.5x_fp16": 2.5,
		// the shared scale clamp still applies
		"up_osaka_99x_fp16": 8,
		"up_osaka_0x_fp16":  1,
	}

	seen := make(map[string]string)

	for id, want := range cases {
		op := parseOne(t, id)

		p, ok := op.(types.Parameterized)
		if !ok {
			t.Fatalf("%q: operation is not Parameterized", id)
		}
		if got, _ := p.Params()[upscale.ParamScale].(float64); got != want {
			t.Errorf("%q: scale = %v, want %v", id, got, want)
		}

		ck, ok := op.(types.CacheKeyer)
		if !ok {
			t.Fatalf("%q: operation is not a CacheKeyer", id)
		}
		if got := ck.CacheKey(); got != upscale.ScaleCacheKey(want) {
			t.Errorf("%q: CacheKey() = %q, want %q", id, got, upscale.ScaleCacheKey(want))
		}

		// Every distinct scale must produce a distinct key, or the shared identity would let one scale serve
		// another's cached result.
		if prev, clash := seen[ck.CacheKey()]; clash && cases[prev] != want {
			t.Errorf("%q and %q share cache key %q with different scales", id, prev, ck.CacheKey())
		}
		seen[ck.CacheKey()] = id
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

// The GUI parses operation ids into operations; opai turns those same ids into models. Each needs a different function
// per model, so the two tables cannot be merged - but they must name the same set of models, and nothing in the
// compiler links them. This is that link.
func TestOperationBuildersMatchLibrary(t *testing.T) {
	want := opai.ModelKeys()

	got := make([]string, 0, len(operationBuilders))
	for key := range operationBuilders {
		got = append(got, key)
	}
	slices.Sort(got)

	if !slices.Equal(got, want) {
		t.Errorf("operationBuilders covers %v, but opai builds %v", got, want)
	}
}

// Selection used to key on the codename alone, which let the two halves of an id disagree without anyone noticing:
// an upscale prefix on a denoise codename quietly built a denoise operation.
func TestIdsToOperationsRejectsMismatchedPrefix(t *testing.T) {
	if _, err := IdsToOperations([]string{"up_stockholm_fp32"}, guitypes.InferenceParams{}); err == nil {
		t.Error("an upscale prefix on a denoise codename should be rejected, not built as a denoise operation")
	}

	if _, err := IdsToOperations([]string{"dn_tokyo_4x_fp32"}, guitypes.InferenceParams{}); err == nil {
		t.Error("a denoise prefix on an upscale codename should be rejected")
	}
}

// Whatever the id's trailing segments carry, the operation it builds must be for the model the first two segments
// name.
func TestIdsToOperationsBuildsTheModelTheIdNames(t *testing.T) {
	ids := []string{"dn_stockholm_0.5_fp32", "sh_moscow_1_fp16", "cl_delhi_fp32", "up_tokyo_4x_fp32", "cb_rio_0.5_fp32"}

	ops, err := IdsToOperations(ids, guitypes.InferenceParams{})
	if err != nil {
		t.Fatalf("IdsToOperations: %v", err)
	}

	if len(ops) != len(ids) {
		t.Fatalf("got %d operations for %d ids", len(ops), len(ids))
	}

	for i, op := range ops {
		if got, want := opai.ModelKey(op.Id()), opai.ModelKey(ids[i]); got != want {
			t.Errorf("id %q built a %q operation, want %q", ids[i], got, want)
		}
	}
}
