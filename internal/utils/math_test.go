package utils

import "testing"

// The 16-multiple rule is a hard requirement of the diffusion upscaler's graph - the VAE compresses 8x and the DiT
// patchifies 2x on top of that - so a misaligned dimension is a shape error at inference time rather than a slightly
// wrong image. FitToMaxSize rounds with this too.
func TestRoundUpTo16(t *testing.T) {
	tests := map[int]int{0: 0, 1: 16, 15: 16, 16: 16, 17: 32, 640: 640, 641: 656, 3000: 3008}

	for in, want := range tests {
		if got := RoundUpTo16(in); got != want {
			t.Fatalf("RoundUpTo16(%d) = %d, want %d", in, got, want)
		}
	}
}
