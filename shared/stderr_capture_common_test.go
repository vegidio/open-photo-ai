package shared

import (
	"bytes"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"strings"
	"sync"
	"testing"
)

const ortWarning = "2026-08-29 10:00:00.123456789 [W:onnxruntime:, session_state.cc:1166 " +
	"VerifyEachNodeIsAssignedToAnEp] Some nodes were not assigned to the preferred execution provider\n"

// startCapture installs a capture that logs into a buffer, and guarantees it comes down again. Nothing here may run in
// parallel: the process's stderr is process-wide, and a capture that outlives its test would swallow the rest of the
// binary's panics and race reports.
func startCapture(t *testing.T) (*stderrCapture, *lockedBuffer) {
	t.Helper()

	buf := &lockedBuffer{}
	logger := slog.New(slog.NewTextHandler(buf, &slog.HandlerOptions{Level: slog.LevelDebug}))

	capture, err := startStderrCapture(logger)
	if err != nil {
		t.Fatalf("failed to start the capture: %v", err)
	}

	t.Cleanup(func() { _ = capture.Close() })

	return capture, buf
}

func TestCaptureRoutesAnOrtLineToTheLogger(t *testing.T) {
	capture, buf := startCapture(t)

	writeStderr(t, ortWarning)

	// Close is the drain barrier: it closes the write end and waits for the reader to reach EOF.
	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	got := buf.String()
	for _, want := range []string{
		"level=WARN",
		"source=onnxruntime",
		"ort_file=session_state.cc",
		"ort_line=1166",
		"ort_func=VerifyEachNodeIsAssignedToAnEp",
		"Some nodes were not assigned",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("the log record is missing %q\ngot: %s", want, got)
		}
	}
}

// TestCaptureKeepsUnrecognizedOutput covers the providers and everything else sharing the descriptor: it is the output
// that used to reach the terminal, so losing it would be a regression.
func TestCaptureKeepsUnrecognizedOutput(t *testing.T) {
	capture, buf := startCapture(t)

	writeStderr(t, "libc++abi: something went sideways\n")

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	got := buf.String()
	if !strings.Contains(got, "source=stderr") || !strings.Contains(got, "sideways") {
		t.Errorf("the unrecognized line was not logged as a plain stderr record\ngot: %s", got)
	}

	if !strings.Contains(got, "level=INFO") {
		t.Errorf("an unrecognized line carries no severity and belongs at INFO\ngot: %s", got)
	}
}

// TestCaptureHandlesAVeryLongLine is the deadlock regression test. A bufio.Scanner gives up on a token past its buffer
// and then stops reading for good; the pipe fills, and the write below never returns. ORT's node-assignment warning
// really does reach this size on a large model.
func TestCaptureHandlesAVeryLongLine(t *testing.T) {
	capture, buf := startCapture(t)

	long := strings.Repeat("n", 512<<10)
	writeStderr(t, fmt.Sprintf("2026-08-29 10:00:00.1 [W:onnxruntime:, x.cc:1 Verify] %s\n", long))

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	got := buf.String()
	if !strings.Contains(got, "truncated,") {
		t.Errorf("the long line was not truncated, so the cap is not being applied")
	}

	if lines := strings.Count(strings.TrimSpace(got), "\n") + 1; lines != 1 {
		t.Errorf("expected exactly one record for one line, got %d", lines)
	}
}

// TestCloseIsIdempotent matters because SetupLogging hands the capture back inside a multiCloser that a caller may
// close alongside its own defer.
func TestCloseIsIdempotent(t *testing.T) {
	capture, _ := startCapture(t)

	if err := capture.Close(); err != nil {
		t.Fatalf("the first close failed: %v", err)
	}

	if err := capture.Close(); err != nil {
		t.Fatalf("the second close failed: %v", err)
	}
}

// TestPanicTraceSurvivesTheCapture guards the one thing the redirect could plausibly destroy.
//
// The runtime writes a crash trace to the process's stderr - now the pipe - after freezing every other goroutine, so
// the reader can never drain it and the trace would die with the process. debug.SetCrashOutput is what keeps it on the
// terminal, and nothing short of an actual crash in an actual child process proves that it does.
func TestPanicTraceSurvivesTheCapture(t *testing.T) {
	if os.Getenv(panicChildEnv) == "1" {
		// The logger deliberately goes nowhere: this child exists to crash, not to log.
		capture, err := startStderrCapture(slog.New(slog.NewTextHandler(io.Discard, nil)))
		if err != nil {
			t.Fatalf("failed to start the capture: %v", err)
		}

		defer capture.Close()

		// The panic has to happen off the test goroutine. tRunner recovers its own, and prints the message through
		// os.Stderr - which the capture preserves - so a panic in the test body would pass this whether or not the
		// crash output is wired up at all. From another goroutine it goes unrecovered to the runtime's crash path,
		// which is the path that writes to the redirected stderr and the one actually under test.
		go panic("the trace for this must reach the terminal")

		select {} // wait to be killed by the panicking goroutine
	}

	cmd := exec.Command(os.Args[0], "-test.run=TestPanicTraceSurvivesTheCapture", "-test.v")
	cmd.Env = append(os.Environ(), panicChildEnv+"=1")

	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	if err := cmd.Run(); err == nil {
		t.Fatal("the child was supposed to panic")
	}

	if got := stderr.String(); !strings.Contains(got, "the trace for this must reach the terminal") {
		t.Errorf("the panic trace was swallowed by the capture:\n%s", got)
	}
}

// panicChildEnv marks the re-executed child of TestPanicTraceSurvivesTheCapture.
const panicChildEnv = "OPAI_TEST_PANIC_CHILD"

// lockedBuffer is a slog sink a test can read while the reader goroutine may still be writing to it.
type lockedBuffer struct {
	mu  sync.Mutex
	buf bytes.Buffer
}

func (b *lockedBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()

	return b.buf.Write(p)
}

func (b *lockedBuffer) String() string {
	b.mu.Lock()
	defer b.mu.Unlock()

	return b.buf.String()
}
