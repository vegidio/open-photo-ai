package shared

import (
	"log/slog"
	"testing"
)

func TestParseOrtLine(t *testing.T) {
	tests := []struct {
		name  string
		line  string
		level slog.Level
		msg   string
		attrs map[string]any
	}{
		{
			name:  "warning with an empty logger id",
			line:  "2026-08-29 10:00:00.123456789 [W:onnxruntime:, session_state.cc:1166 VerifyEachNodeIsAssignedToAnEp] Some nodes were not assigned",
			level: slog.LevelWarn,
			msg:   "Some nodes were not assigned",
			attrs: map[string]any{
				"source":   "onnxruntime",
				"ort_file": "session_state.cc",
				"ort_line": 1166,
				"ort_func": "VerifyEachNodeIsAssignedToAnEp",
			},
		},
		{
			name:  "the environment name is not worth an attribute",
			line:  "2026-08-29 10:00:00.1 [I:onnxruntime:Golang onnxruntime environment, inference_session.cc:500 Init] Starting",
			level: slog.LevelInfo,
			msg:   "Starting",
			attrs: map[string]any{"source": "onnxruntime", "ort_file": "inference_session.cc", "ort_line": 500, "ort_func": "Init"},
		},
		{
			name:  "a session log id is kept",
			line:  "2026-08-29 10:00:00.1 [V:onnxruntime:osaka-fp16, allocator.cc:12 Alloc] reserving",
			level: slog.LevelDebug,
			msg:   "reserving",
			attrs: map[string]any{"source": "onnxruntime", "ort_logger": "osaka-fp16", "ort_file": "allocator.cc", "ort_line": 12, "ort_func": "Alloc"},
		},
		{
			name:  "fatal is flagged because slog has no level for it",
			line:  "2026-08-29 10:00:00.1 [F:onnxruntime:, env.cc:9 Die] out of memory",
			level: slog.LevelError,
			msg:   "out of memory",
			attrs: map[string]any{"source": "onnxruntime", "ort_fatal": true, "ort_file": "env.cc", "ort_line": 9, "ort_func": "Die"},
		},
		{
			name:  "error severity is not flagged as fatal",
			line:  "2026-08-29 10:00:00.1 [E:onnxruntime:, env.cc:9 Run] failed",
			level: slog.LevelError,
			msg:   "failed",
			attrs: map[string]any{"source": "onnxruntime", "ort_file": "env.cc", "ort_line": 9, "ort_func": "Run"},
		},
		{
			name:  "a message may contain brackets of its own",
			line:  "2026-08-29 10:00:00.1 [W:onnxruntime:, x.cc:1 F] input [1,3,64,64] rejected",
			level: slog.LevelWarn,
			msg:   "input [1,3,64,64] rejected",
			attrs: map[string]any{"source": "onnxruntime", "ort_file": "x.cc", "ort_line": 1, "ort_func": "F"},
		},
		{
			name:  "the function name may be missing",
			line:  "2026-08-29 10:00:00.1 [W:onnxruntime:, coreml_execution_provider.cc:88] unsupported op",
			level: slog.LevelWarn,
			msg:   "unsupported op",
			attrs: map[string]any{"source": "onnxruntime", "ort_file": "coreml_execution_provider.cc", "ort_line": 88},
		},
		{
			// Verbatim from a CLI run: once stderr is a pipe rather than a terminal, macOS prepends its own
			// timestamp, process name, pid and thread id. Missing this is what made every real ORT line parse as
			// unrecognized output and arrive at INFO with no severity.
			name:  "the macOS log prefix is stripped",
			line:  "2026-08-29 14:31:40.145 cli[22667:2454974] 2026-08-29 14:31:40.138303 [W:onnxruntime:, session_state.cc:1367 VerifyEachNodeIsAssignedToAnEp] Some nodes were not assigned",
			level: slog.LevelWarn,
			msg:   "Some nodes were not assigned",
			attrs: map[string]any{
				"source":   "onnxruntime",
				"ort_file": "session_state.cc",
				"ort_line": 1367,
				"ort_func": "VerifyEachNodeIsAssignedToAnEp",
			},
		},
		{
			name:  "a full source path is reduced to the file name",
			line:  "2026-08-29 10:00:00.1 [W:onnxruntime:, /build/onnxruntime/core/session/x.cc:7 Go] hi",
			level: slog.LevelWarn,
			msg:   "hi",
			attrs: map[string]any{"source": "onnxruntime", "ort_file": "x.cc", "ort_line": 7, "ort_func": "Go"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			rec, ok := parseOrtLine(tt.line)
			if !ok {
				t.Fatalf("the line was not recognized as ORT output: %s", tt.line)
			}

			if rec.level != tt.level {
				t.Errorf("level = %v, want %v", rec.level, tt.level)
			}

			if rec.msg != tt.msg {
				t.Errorf("msg = %q, want %q", rec.msg, tt.msg)
			}

			got := attrMap(t, rec.attrs)
			for k, want := range tt.attrs {
				if got[k] != want {
					t.Errorf("attr %s = %#v, want %#v", k, got[k], want)
				}
			}

			for k := range got {
				if _, expected := tt.attrs[k]; !expected {
					t.Errorf("unexpected attr %s = %#v", k, got[k])
				}
			}
		})
	}
}

// TestParseOrtLineRejectsOtherOutput covers what shares the descriptor: the providers' own logging, continuation lines
// of a multi-line ORT message, and anything else in the process that writes to stderr. None of it may be mistaken for
// a parsed record, because emit() gives those lines a different treatment.
func TestParseOrtLineRejectsOtherOutput(t *testing.T) {
	lines := []string{
		"",
		"   ",
		"[W:onnxruntime:, x.cc:1 F] no timestamp",
		"2026-08-29 10:00:00.1 [X:onnxruntime:, x.cc:1 F] unknown severity letter",
		"2026-08-29 10:00:00 [W:onnxruntime:, x.cc:1 F] no fractional seconds",
		"    at frame 3 (continuation of the previous message)",
		"2026-08-29 10:00:00.1 W:onnxruntime: x.cc:1 no brackets",
	}

	for _, line := range lines {
		if _, ok := parseOrtLine(line); ok {
			t.Errorf("parsed a line that is not ORT output: %q", line)
		}
	}
}

// TestNormalizeLineFeedsTheParser is the point of the whole exercise: a Windows line has to come out the far end as a
// warning, not as unrecognized output.
func TestNormalizeLineFeedsTheParser(t *testing.T) {
	wide, _ := utf16Line("\x1b[0;93m2026-08-29 10:00:00.1 [W:onnxruntime:, session_state.cc:1367 Verify] fell back\x1b[m")

	rec, ok := parseOrtLine(normalizeLine(wide))
	if !ok {
		t.Fatal("a colourized wide line did not parse as ONNX Runtime output")
	}

	if rec.level != slog.LevelWarn || rec.msg != "fell back" {
		t.Errorf("got level %v and message %q", rec.level, rec.msg)
	}
}

// attrMap flattens slog's alternating key/value slice so a test can assert on it by name.
func attrMap(t *testing.T, attrs []any) map[string]any {
	t.Helper()

	if len(attrs)%2 != 0 {
		t.Fatalf("attrs must be key/value pairs, got %d elements", len(attrs))
	}

	out := make(map[string]any, len(attrs)/2)
	for i := 0; i < len(attrs); i += 2 {
		key, ok := attrs[i].(string)
		if !ok {
			t.Fatalf("attr key %d is not a string: %#v", i, attrs[i])
		}

		out[key] = attrs[i+1]
	}

	return out
}
