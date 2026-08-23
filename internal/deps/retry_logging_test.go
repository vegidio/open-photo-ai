package deps

import (
	"bytes"
	"context"
	"log/slog"
	"net/http"
	"strings"
	"sync"
	"testing"

	"github.com/vegidio/open-photo-ai/internal"
)

// captureLog points the library logger at a buffer for the duration of a test and returns a reader for what was
// written. The library defaults to a discard handler, so without this every assertion below would pass vacuously.
func captureLog(t *testing.T) func() string {
	t.Helper()

	var (
		mu  sync.Mutex
		buf bytes.Buffer
	)

	// The retry loop's stall test runs its op on another goroutine, so the buffer needs its own lock: slog serializes
	// handler calls, not the writer it was handed.
	writer := writerFunc(func(p []byte) (int, error) {
		mu.Lock()
		defer mu.Unlock()

		return buf.Write(p)
	})

	original := internal.Log()
	internal.SetLogger(slog.New(slog.NewTextHandler(writer, &slog.HandlerOptions{Level: slog.LevelDebug})))

	t.Cleanup(func() { internal.SetLogger(original) })

	return func() string {
		mu.Lock()
		defer mu.Unlock()

		return buf.String()
	}
}

type writerFunc func([]byte) (int, error)

func (f writerFunc) Write(p []byte) (int, error) { return f(p) }

// The download path is the slowest thing the app does and the likeliest to fail on a user's machine, and the only
// artifact anyone sends us afterwards is the log file. That makes these lines the feature rather than a side effect,
// which is why asserting on their contents is worth the brittleness.
func TestWithRetryReportsWhyItGaveUp(t *testing.T) {
	fastRetries(t)

	logged := captureLog(t)

	err := withRetry(t.Context(), "onnx-runtime.7z", func(int) (int64, error) { return 0, errStalled })
	if err == nil {
		t.Fatal("expected withRetry to give up")
	}

	out := logged()

	// A stall is indistinguishable from an ordinary dropped connection in the error text alone, and "the progress bar
	// froze" is exactly how users describe it.
	if !strings.Contains(out, "stalled=true") {
		t.Errorf("retry log does not mark the stall:\n%s", out)
	}

	if !strings.Contains(out, "onnx-runtime.7z") {
		t.Errorf("retry log does not name the artifact:\n%s", out)
	}

	if !strings.Contains(out, "gave up on the transfer") {
		t.Errorf("retry log does not record giving up:\n%s", out)
	}

	// Whether the file was converging at all is the difference between "bad link" and "wrong URL".
	if !strings.Contains(out, "fruitless=") || !strings.Contains(out, "furthest=") {
		t.Errorf("retry log does not say how far the transfer got:\n%s", out)
	}
}

// A 404 from a mis-generated URL is not retryable, so it leaves through a different branch than the one above - and it
// used to leave without a line at any level.
func TestWithRetryReportsANonRetryableFailure(t *testing.T) {
	fastRetries(t)

	logged := captureLog(t)

	notFound := &httpStatusError{StatusCode: http.StatusNotFound, Status: "404 Not Found"}

	if err := withRetry(t.Context(), "model.onnx", func(int) (int64, error) { return 0, notFound }); err == nil {
		t.Fatal("expected withRetry to fail")
	}

	out := logged()

	if !strings.Contains(out, "will not be retried") {
		t.Errorf("a non-retryable failure was not reported:\n%s", out)
	}

	if !strings.Contains(out, "model.onnx") {
		t.Errorf("the non-retryable failure does not name the artifact:\n%s", out)
	}
}

// Cancelling an install is a decision, not a fault, so it must not be reported as a failure.
func TestWithRetryReportsCancellationAsSuch(t *testing.T) {
	fastRetries(t)

	logged := captureLog(t)

	ctx, cancel := context.WithCancel(t.Context())
	cancel()

	if err := withRetry(ctx, "cuda.7z", func(int) (int64, error) { return 0, errStalled }); err == nil {
		t.Fatal("expected withRetry to stop")
	}

	out := logged()

	if !strings.Contains(out, "transfer cancelled") {
		t.Errorf("a cancelled transfer was not reported as cancelled:\n%s", out)
	}

	if strings.Contains(out, "gave up on the transfer") {
		t.Errorf("a cancelled transfer was reported as a failure:\n%s", out)
	}
}

// A transfer that succeeds must stay silent: the caller already reports the install, and a line per successful
// attempt would bury the ones that matter.
func TestWithRetrySaysNothingOnSuccess(t *testing.T) {
	fastRetries(t)

	logged := captureLog(t)

	if err := withRetry(t.Context(), "quiet.7z", func(int) (int64, error) { return 100, nil }); err != nil {
		t.Fatalf("withRetry failed: %v", err)
	}

	if out := logged(); out != "" {
		t.Errorf("the success path logged:\n%s", out)
	}
}

// Guards the retry budget's own accounting through the logging change: an attempt that advanced resets the count, so
// the artifact keeps its full budget from wherever it last got further.
func TestWithRetryLogsEveryAttemptItRetries(t *testing.T) {
	fastRetries(t)

	logged := captureLog(t)

	var reached int64

	_ = withRetry(t.Context(), "advancing.7z", func(int) (int64, error) {
		reached += 10
		if reached >= 50 {
			return reached, nil
		}

		return reached, errStalled
	})

	if got := strings.Count(logged(), "transfer attempt failed; retrying"); got != 4 {
		t.Errorf("logged %d retry lines, want 4 (one per failed attempt before the success)", got)
	}
}
