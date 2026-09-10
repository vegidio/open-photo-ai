package shared

import (
	"bytes"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
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

	capture, err := startStderrCapture(logger, filepath.Join(t.TempDir(), "native.log"))
	if err != nil {
		t.Fatalf("failed to start the capture: %v", err)
	}

	t.Cleanup(func() { _ = capture.Close() })

	return capture, buf
}

func TestCaptureRoutesAnOrtLineToTheLogger(t *testing.T) {
	capture, buf := startCapture(t)

	writeStderr(t, ortWarning)

	// Close is the barrier: it restores stderr, then has the tailer read the file to the end before returning.
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

// TestCaptureHandlesAVeryLongLine keeps one line one record, however long it runs. ORT's node-assignment warning
// really does reach this size on a large model, and the cap that applies is truncate's, at formatting time - never a
// split into several records, which would be unreadable exactly where the detail matters.
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
		capture, err := startStderrCapture(slog.New(slog.NewTextHandler(io.Discard, nil)),
			filepath.Join(t.TempDir(), "native.log"))
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

// TestNativeOutputSurvivesAnUncleanExit is the regression test for issue #40, and the reason this capture writes to a
// file rather than a pipe.
//
// The reporter's crash killed the process from inside a CUDA session. Whatever ORT wrote on its way down was still in
// the pipe, unread, when every goroutine stopped - so the log they attached simply ended mid-session and the failure
// was invisible. The child below reproduces the shape that matters: write to stderr, then leave without closing the
// capture or draining anything. Only a real second process proves it; a Close in the same process would drain the
// file and pass whether or not the bytes had ever reached disk.
func TestNativeOutputSurvivesAnUncleanExit(t *testing.T) {
	if path := os.Getenv(crashChildEnv); path != "" {
		// The logger goes nowhere on purpose: this child exists to die, and the assertion is about the file.
		if _, err := startStderrCapture(slog.New(slog.NewTextHandler(io.Discard, nil)), path); err != nil {
			t.Fatalf("failed to start the capture: %v", err)
		}

		writeStderr(t, "libc++abi: terminating due to uncaught exception of type onnxruntime::OnnxRuntimeException\n")

		// Straight out: no Close, no drain, nothing flushed - the shape of a native crash.
		os.Exit(3)
	}

	// Not t.TempDir(): the child must outlive nothing here, but the path has to survive into its environment, and a
	// directory the parent owns is removed only after the read below.
	path := filepath.Join(t.TempDir(), "native.log")

	cmd := exec.Command(os.Args[0], "-test.run=TestNativeOutputSurvivesAnUncleanExit")
	cmd.Env = append(os.Environ(), crashChildEnv+"="+path)

	if err := cmd.Run(); err == nil {
		t.Fatal("the child was supposed to exit non-zero")
	}

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("the capture file is gone after the child died, so nothing reached disk: %v", err)
	}

	if !strings.Contains(string(data), "uncaught exception") {
		t.Errorf("the native line did not survive the unclean exit\ngot: %q", data)
	}
}

// crashChildEnv carries the capture path into the re-executed child of TestNativeOutputSurvivesAnUncleanExit.
const crashChildEnv = "OPAI_TEST_CRASH_CHILD"

// TestFoldNativeLogReplaysAPreviousRun covers the other half: reaching disk is only useful if the next run picks it
// up and puts it where a bug report will find it.
func TestFoldNativeLogReplaysAPreviousRun(t *testing.T) {
	path := filepath.Join(t.TempDir(), "native.log")

	if err := os.WriteFile(path, []byte(ortWarning+"libc++abi: terminating\n"), 0o600); err != nil {
		t.Fatalf("failed to seed the native log: %v", err)
	}

	buf := &lockedBuffer{}
	foldNativeLog(slog.New(slog.NewTextHandler(buf, &slog.HandlerOptions{Level: slog.LevelDebug})), path)

	got := buf.String()
	for _, want := range []string{
		"did not exit cleanly",
		"source=onnxruntime",
		"Some nodes were not assigned",
		"libc++abi: terminating",
		"previous_run=true",
		"end of the previous run",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("the replayed log is missing %q\ngot: %s", want, got)
		}
	}

	// Left in place it would be replayed again on every subsequent launch.
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Errorf("the native log was not removed after being replayed (err=%v)", err)
	}
}

// TestFoldNativeLogIgnoresACleanStart pins the quiet path: no file, or an empty one, must say nothing at all. A
// warning on every normal launch would train the reader to skip the one launch where it means something.
func TestFoldNativeLogIgnoresACleanStart(t *testing.T) {
	dir := t.TempDir()

	for _, tt := range []struct{ name, file string }{
		{"no file at all", "missing.log"},
		{"an empty file", "empty.log"},
	} {
		t.Run(tt.name, func(t *testing.T) {
			path := filepath.Join(dir, tt.file)
			if tt.file == "empty.log" {
				if err := os.WriteFile(path, nil, 0o600); err != nil {
					t.Fatalf("failed to seed the native log: %v", err)
				}
			}

			buf := &lockedBuffer{}
			foldNativeLog(slog.New(slog.NewTextHandler(buf, nil)), path)

			if got := buf.String(); got != "" {
				t.Errorf("a clean start logged something: %s", got)
			}
		})
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
