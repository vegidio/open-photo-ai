package opai

import (
	"bytes"
	"errors"
	"log/slog"
	"strings"
	"testing"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

type fakeOperation struct{ id string }

func (o fakeOperation) Id() string                 { return o.id }
func (o fakeOperation) Precision() types.Precision { return types.PrecisionFp32 }

// TestLogModelRunReportsFailures covers the one line that used to lie.
//
// logModelRun was called unconditionally after Model.Run and always wrote "model run complete", so a model that
// errored produced a log saying it had finished successfully - and the run's duration, the only thing that
// distinguishes a fast rejection from a timeout, was attached to that false claim. Nothing else in the library logged
// the failure, so a failed enhancement left the log reading like a successful one.
func TestLogModelRunReportsFailures(t *testing.T) {
	tests := []struct {
		name      string
		err       error
		wantLevel string
		wantMsg   string
	}{
		{"success", nil, "level=INFO", "model run complete"},
		{"failure", errors.New("ORT session failed"), "level=WARN", "model run failed"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var buf bytes.Buffer

			original := internal.Log()
			internal.SetLogger(slog.New(slog.NewTextHandler(&buf, &slog.HandlerOptions{Level: slog.LevelDebug})))
			t.Cleanup(func() { internal.SetLogger(original) })

			logModelRun(fakeOperation{id: "up-tokyo"}, time.Now(), tt.err)

			out := buf.String()

			if !strings.Contains(out, tt.wantMsg) {
				t.Errorf("got %q, want it to contain %q", out, tt.wantMsg)
			}

			if !strings.Contains(out, tt.wantLevel) {
				t.Errorf("got %q, want it logged at %s", out, tt.wantLevel)
			}

			// The operation is what tells you which of the seven steps in a chain this was.
			if !strings.Contains(out, "op=up-tokyo") {
				t.Errorf("got %q, want it to name the operation", out)
			}

			// The duration is the point of the line in both directions.
			if !strings.Contains(out, "duration=") {
				t.Errorf("got %q, want it to carry the duration", out)
			}

			if tt.err != nil && !strings.Contains(out, "ORT session failed") {
				t.Errorf("got %q, want it to carry the error", out)
			}

			// The failure case must not also claim success.
			if tt.err != nil && strings.Contains(out, "model run complete") {
				t.Errorf("a failed run was reported as complete: %q", out)
			}
		})
	}
}
