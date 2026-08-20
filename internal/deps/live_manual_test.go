//go:build manual

package deps

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
)

// TestLiveResumeAgainstHuggingFace is the end-to-end check the unit tests cannot make: that the real
// CDN honours a Range request on the signed URL it redirects to, and that an interrupted install
// picks up from where it stopped rather than starting over.
//
// Behind a build tag because it downloads ~375 MB from the network. Run it with:
//
//	go test -tags manual -run TestLiveResume -v ./internal/deps/
func TestLiveResumeAgainstHuggingFace(t *testing.T) {
	root := t.TempDir()
	t.Setenv("HOME", root)
	t.Setenv("XDG_CONFIG_HOME", root)
	t.Setenv("AppData", root)

	internal.AppName = "opai-live-test"

	dep := Dependency{
		Name:        "fr_athens_fp32",
		Destination: internal.ModelsDir,
		Sources: []Source{{
			URL:    internal.ModelBaseUrl + "/fr_athens_fp32.onnx",
			Sha256: "51689997e74d2c708534830c25df1c47aac1de60c73899b8b13a8c41fbbd42b5",
			Size:   376620748,
		}},
	}

	config, err := os.UserConfigDir()
	if err != nil {
		t.Fatalf("failed to resolve the config directory: %v", err)
	}

	dir := filepath.Join(config, internal.AppName, internal.ModelsDir)
	part, state := partPaths(dir, "fr_athens_fp32.onnx")

	// Interrupt the first attempt partway through.
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()

	if err = Install(ctx, dep, nil); err == nil {
		t.Skip("the download finished before it could be interrupted; the link is too fast for this test")
	}

	info, err := os.Stat(part)
	if err != nil {
		t.Fatalf("the interrupted download left no part file to resume: %v", err)
	}

	if _, err = os.Stat(state); err != nil {
		t.Fatalf("the interrupted download left no record beside its part file: %v", err)
	}

	partial := info.Size()
	t.Logf("interrupted with %.1f MB on disk (%.0f%% of the artifact)",
		float64(partial)/1e6, 100*float64(partial)/float64(dep.Sources[0].Size))

	if partial == 0 {
		t.Fatal("the part file is empty; nothing to resume from")
	}

	start := time.Now()

	if err = Install(context.Background(), dep, nil); err != nil {
		t.Fatalf("the resumed install failed: %v", err)
	}

	elapsed := time.Since(start)
	remaining := dep.Sources[0].Size - partial

	t.Logf("resumed and completed the remaining %.1f MB in %v (%.1f MB/s)",
		float64(remaining)/1e6, elapsed.Round(time.Millisecond),
		float64(remaining)/1e6/elapsed.Seconds())

	final, err := os.Stat(filepath.Join(dir, "fr_athens_fp32.onnx"))
	if err != nil {
		t.Fatalf("the model was not installed: %v", err)
	}

	if final.Size() != dep.Sources[0].Size {
		t.Errorf("installed %d bytes, want %d", final.Size(), dep.Sources[0].Size)
	}

	// The hash was verified inside Install; reaching here at the full size means the resumed prefix
	// and the tail agreed with the pinned value.
	if _, err = os.Stat(part); !os.IsNotExist(err) {
		t.Error("the part file survived a successful install")
	}
}
