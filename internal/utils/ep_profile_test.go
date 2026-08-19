package utils

import (
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

func TestCoreMLOptionsFollowTheProfile(t *testing.T) {
	tests := []struct {
		name    string
		profile EPProfile
		want    string
	}{
		{"the zero value keeps the shipped behaviour", EPProfile{}, "1"},
		{"a dynamic-shape model relaxes it", EPProfile{DynamicShapes: true}, "0"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := coreMLOptions("/cache", tt.profile)

			if got["RequireStaticInputShapes"] != tt.want {
				t.Fatalf("RequireStaticInputShapes = %q, want %q", got["RequireStaticInputShapes"], tt.want)
			}
			if got["ModelCacheDirectory"] != "/cache" {
				t.Fatalf("ModelCacheDirectory = %q", got["ModelCacheDirectory"])
			}
			if got["ModelFormat"] != "MLProgram" {
				t.Fatalf("ModelFormat = %q", got["ModelFormat"])
			}
		})
	}
}

func TestTensorRTOptionsFollowTheProfile(t *testing.T) {
	zero := tensorRTOptions("/cache", EPProfile{})

	if zero["trt_max_workspace_size"] != "4294967296" {
		t.Fatalf("default workspace = %q", zero["trt_max_workspace_size"])
	}
	if zero["trt_fp16_enable"] != "0" {
		t.Fatalf("fp16 must be opt-in, got %q", zero["trt_fp16_enable"])
	}

	tuned := tensorRTOptions("/cache", EPProfile{
		Fp16:              true,
		TrtWorkspaceBytes: 1 << 30,
		TrtShapes:         map[string]string{"trt_profile_min_shapes": "vid_input:1x33x32x32"},
	})

	if tuned["trt_fp16_enable"] != "1" {
		t.Fatalf("fp16 not applied: %q", tuned["trt_fp16_enable"])
	}
	if tuned["trt_max_workspace_size"] != "1073741824" {
		t.Fatalf("workspace override not applied: %q", tuned["trt_max_workspace_size"])
	}
	if tuned["trt_profile_min_shapes"] != "vid_input:1x33x32x32" {
		t.Fatalf("shape profile not merged: %q", tuned["trt_profile_min_shapes"])
	}
}

func TestResolveProviders(t *testing.T) {
	trt := types.ExecutionProviderTensorRT
	cuda := types.ExecutionProviderCUDA
	coreml := types.ExecutionProviderCoreML
	directml := types.ExecutionProviderDirectML

	tests := []struct {
		name    string
		goos    string
		ep      types.ExecutionProvider
		profile EPProfile
		want    []types.ExecutionProvider
		wantErr bool
	}{
		{
			name: "an explicit request yields just that provider",
			goos: "linux", ep: cuda,
			want: []types.ExecutionProvider{cuda},
		},
		{
			name: "auto walks the platform chain",
			goos: "darwin", ep: types.ExecutionProviderAuto,
			want: []types.ExecutionProvider{coreml, types.ExecutionProviderOpenVINO},
		},
		{
			name: "an excluded provider is dropped from auto",
			goos: "linux", ep: types.ExecutionProviderAuto,
			profile: EPProfile{ExcludeEPs: []types.ExecutionProvider{trt}},
			want:    []types.ExecutionProvider{cuda, types.ExecutionProviderOpenVINO},
		},
		{
			name: "an excluded explicit request falls to the rest of the chain, never to the CPU",
			goos: "linux", ep: trt,
			profile: EPProfile{ExcludeEPs: []types.ExecutionProvider{trt}},
			want:    []types.ExecutionProvider{cuda, types.ExecutionProviderOpenVINO},
		},
		{
			name: "a provider the platform lacks is an error",
			goos: "linux", ep: coreml,
			wantErr: true,
		},
		{
			name: "DirectML is Windows-only",
			goos: "linux", ep: directml,
			wantErr: true,
		},
		{
			name: "an unknown platform is an error",
			goos: "plan9", ep: cuda,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := resolveProviders(tt.goos, tt.ep, tt.profile)

			if tt.wantErr {
				if err == nil {
					t.Fatalf("want an error, got %v", got)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			if len(got) != len(tt.want) {
				t.Fatalf("want %v, got %v", tt.want, got)
			}
			for i := range got {
				if got[i] != tt.want[i] {
					t.Fatalf("want %v, got %v", tt.want, got)
				}
			}
		})
	}
}

// Every provider the platform chains reference must have an appender, or createOptions would panic on a nil map
// entry the first time that platform selected it.
func TestEveryChainedProviderHasAnAppender(t *testing.T) {
	for goos, chain := range autoChain {
		for _, ep := range chain {
			if providerAppenders[ep] == nil {
				t.Fatalf("%s chains %s, which has no appender", goos, ep)
			}
		}
	}
}
