//go:build darwin || linux

package shared

import (
	"os"
	"strings"
	"testing"

	"golang.org/x/sys/unix"
)

// writeStderr writes to descriptor 2 directly. An os.File wrapper is not an option: os.NewFile takes ownership, and
// its finalizer would close the test binary's own stderr.
func writeStderr(t *testing.T, s string) {
	t.Helper()

	for written := 0; written < len(s); {
		n, err := unix.Write(unix.Stderr, []byte(s[written:]))
		if err != nil {
			t.Fatalf("failed to write to stderr: %v", err)
		}

		written += n
	}
}

// TestGoStderrSurvivesTheCapture: the redirect must not swallow the application's own prints and panic traces, which
// is what keeps a crash visible in a terminal.
func TestGoStderrSurvivesTheCapture(t *testing.T) {
	capture, buf := startCapture(t)

	if os.Stderr.Fd() == uintptr(unix.Stderr) {
		t.Error("os.Stderr still points at descriptor 2, so Go's own output is going into the pipe")
	}

	const marker = "this line belongs on the terminal, not in the log file\n"
	if _, err := os.Stderr.WriteString(marker); err != nil {
		t.Fatalf("failed to write to the preserved stderr: %v", err)
	}

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	if strings.Contains(buf.String(), "belongs on the terminal") {
		t.Errorf("a write to os.Stderr was captured, so the original descriptor was not preserved:\n%s", buf.String())
	}
}

func TestCloseRestoresStderr(t *testing.T) {
	var before, after unix.Stat_t

	if err := unix.Fstat(unix.Stderr, &before); err != nil {
		t.Fatalf("failed to stat stderr: %v", err)
	}

	capture, _ := startCapture(t)

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	if err := unix.Fstat(unix.Stderr, &after); err != nil {
		t.Fatalf("failed to stat stderr: %v", err)
	}

	if before.Dev != after.Dev || before.Ino != after.Ino {
		t.Error("descriptor 2 does not point at what it did before the capture was installed")
	}
}
