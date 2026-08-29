package shared

import (
	"strings"
	"testing"
)

// utf16Line encodes what ONNX Runtime's std::wcerr puts on the pipe on Windows: the line as UTF-16LE, ending in a
// wide newline whose low byte is the one the reader splits on.
func utf16Line(s string) (string, string) {
	var wide strings.Builder

	for _, r := range s + "\n" {
		wide.WriteByte(byte(r))
		wide.WriteByte(byte(r >> 8))
	}

	// The reader cuts after the newline's low byte, so the high byte opens the line that follows.
	encoded := wide.String()

	return encoded[:len(encoded)-1], encoded[len(encoded)-1:]
}

func TestNormalizeLine(t *testing.T) {
	const warning = "2026-08-29 10:00:00.1 [W:onnxruntime:, x.cc:1 Verify] Some nodes were not assigned"

	wide, leftover := utf16Line(warning)

	tests := []struct {
		name string
		line string
		want string
	}{
		{name: "a plain line is left alone", line: warning, want: warning},
		{name: "the trailing newline goes", line: warning + "\r\n", want: warning},
		{
			name: "ONNX Runtime's colour codes are stripped",
			line: "\x1b[0;93m" + warning + "\x1b[m",
			want: warning,
		},
		{name: "a wide line is decoded", line: wide, want: warning},
		{
			name: "a wide line keeps its colour stripped too",
			line: func() string { l, _ := utf16Line("\x1b[0;93m" + warning + "\x1b[m"); return l }(),
			want: warning,
		},
		{name: "the leftover half of a wide newline is dropped", line: leftover, want: ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := normalizeLine(tt.line); got != tt.want {
				t.Errorf("normalizeLine(%q) = %q, want %q", tt.line, got, tt.want)
			}
		})
	}
}

func TestTruncate(t *testing.T) {
	short := strings.Repeat("a", maxLoggedLine)
	if got := truncate(short); got != short {
		t.Errorf("a message at the cap must be left alone, got %d bytes", len(got))
	}

	long := strings.Repeat("a", maxLoggedLine+100)
	got := truncate(long)

	if !strings.HasSuffix(got, "… (truncated, 16484 bytes)") {
		t.Errorf("missing the truncation marker: %q", got[len(got)-40:])
	}

	if !strings.HasPrefix(got, short) {
		t.Error("the retained prefix is not the start of the message")
	}
}

// TestTruncateCutsOnARuneBoundary guards against splitting a multi-byte rune, which would put a replacement character
// in the log file.
func TestTruncateCutsOnARuneBoundary(t *testing.T) {
	// "é" is two bytes, so a cut at maxLoggedLine lands mid-rune.
	got := truncate(strings.Repeat("a", maxLoggedLine-1) + strings.Repeat("é", 10))

	if !strings.HasPrefix(got, strings.Repeat("a", maxLoggedLine-1)+"… ") {
		t.Errorf("the cut did not fall back to the rune boundary: %q", got[maxLoggedLine-5:])
	}
}
