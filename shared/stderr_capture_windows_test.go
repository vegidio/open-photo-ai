//go:build windows

package shared

import (
	"os"
	"runtime"
	"strings"
	"testing"
	"unsafe"

	"golang.org/x/sys/windows"
)

var (
	procWrite  = ucrtbase.NewProc("_write")
	procFputws = ucrtbase.NewProc("fputws")
	procFflush = ucrtbase.NewProc("fflush")
)

// writeStderr writes to descriptor 2 through the same C runtime ONNX Runtime's own writes go through. An os.File
// wrapper is not an option: os.NewFile takes ownership, and its finalizer would close the test binary's own stderr.
func writeStderr(t *testing.T, s string) {
	t.Helper()

	b := []byte(s)

	for written := 0; written < len(b); {
		r, _, _ := procWrite.Call(uintptr(stderrFd), uintptr(unsafe.Pointer(&b[written])), uintptr(len(b)-written))
		runtime.KeepAlive(b)

		n := int(int32(r))
		if n <= 0 {
			t.Fatalf("failed to write to stderr, _write returned %d", n)
		}

		written += n
	}
}

// TestCaptureRoutesAWideOrtLine is the Windows shape of the problem. ONNX Runtime logs through std::wcerr, so what
// arrives is the stderr stream's own descriptor carrying UTF-16, one NUL per ASCII character, colour codes and all -
// and it still has to come out as a warning rather than as unreadable bytes at INFO.
func TestCaptureRoutesAWideOrtLine(t *testing.T) {
	capture, buf := startCapture(t)

	writeWideStderr(t, "\x1b[0;93m"+strings.TrimSuffix(ortWarning, "\n")+"\x1b[m\n")

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	got := buf.String()
	for _, want := range []string{"level=WARN", "source=onnxruntime", "Some nodes were not assigned"} {
		if !strings.Contains(got, want) {
			t.Errorf("the wide log record is missing %q\ngot: %s", want, got)
		}
	}

	if strings.Contains(got, `\x00`) || strings.Contains(got, `\x1b`) {
		t.Errorf("the record still carries the wide encoding or the colour codes\ngot: %s", got)
	}
}

// TestStderrStreamIsRedirected covers the descriptor the C runtime's stderr stream sits on, which is not always
// descriptor 2: a GUI-subsystem process starts with a detached stream that has no descriptor at all, and ORT's writes
// go nowhere unless the capture gives it one.
func TestStderrStreamIsRedirected(t *testing.T) {
	capture, buf := startCapture(t)

	stream := stderrStream()
	if stream == 0 {
		t.Fatal("the C runtime's stderr stream could not be reached")
	}

	if fd := streamFd(stream); fd < 0 {
		t.Fatalf("the stderr stream has no descriptor after the redirect, so its writes are discarded: %d", fd)
	}

	writeWideStderr(t, "PROBE-THROUGH-THE-STREAM\n")

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	if !strings.Contains(buf.String(), "PROBE-THROUGH-THE-STREAM") {
		t.Errorf("a write through the stderr stream was not captured\ngot: %s", buf.String())
	}
}

// TestGoStderrSurvivesTheCapture: the redirect must not swallow the application's own prints and panic traces, which
// is what keeps a crash visible in a terminal. On Windows that means os.Stderr has to end up on a duplicate, because
// pointing descriptor 2 at the pipe closes the handle it held.
func TestGoStderrSurvivesTheCapture(t *testing.T) {
	capture, buf := startCapture(t)

	const marker = "this line belongs on the terminal, not in the log file\n"
	if _, err := os.Stderr.WriteString(marker); err != nil {
		t.Fatalf("failed to write to the preserved stderr: %v", err)
	}

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	if strings.Contains(buf.String(), "belongs on the terminal") {
		t.Errorf("a write to os.Stderr was captured, so the original was not preserved:\n%s", buf.String())
	}
}

func TestCloseRestoresStderr(t *testing.T) {
	capture, buf := startCapture(t)

	if err := capture.Close(); err != nil {
		t.Fatalf("failed to close the capture: %v", err)
	}

	// Nothing written from here on belongs to the capture: descriptor 2 is the terminal's again, which is where this
	// deliberately noisy line goes.
	writeStderr(t, "this line is written after the capture was closed\n")

	if strings.Contains(buf.String(), "after the capture was closed") {
		t.Errorf("descriptor 2 still feeds the pipe after Close\ngot: %s", buf.String())
	}
}

// writeWideStderr writes through the C runtime's stderr stream with wide characters - ONNX Runtime's own route.
func writeWideStderr(t *testing.T, s string) {
	t.Helper()

	stream := stderrStream()
	if stream == 0 {
		t.Fatal("the C runtime's stderr stream could not be reached")
	}

	wide, err := windows.UTF16PtrFromString(s)
	if err != nil {
		t.Fatalf("failed to encode the line: %v", err)
	}

	r, _, _ := procFputws.Call(uintptr(unsafe.Pointer(wide)), stream)
	runtime.KeepAlive(wide)

	if int(int32(r)) < 0 {
		t.Fatal("failed to write to the stderr stream")
	}

	procFflush.Call(stream)
}
