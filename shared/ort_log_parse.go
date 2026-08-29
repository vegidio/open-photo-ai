package shared

import (
	"log/slog"
	"path"
	"regexp"
	"strconv"
	"strings"
)

// ortLineRe matches one line of ONNX Runtime's default (ostream) log sink:
//
//	2026-08-29 10:00:00.123456789 [W:onnxruntime:, session_state.cc:1166 VerifyEachNodeIsAssignedToAnEp] message
//
// The fields are timestamp, severity letter, category, logger id, code location and message. The logger id is whatever
// name was passed to CreateEnv ("Golang onnxruntime environment" for this binding), or a session's log id, or empty -
// so it is allowed to contain spaces and to be absent. The message may itself contain "]", hence the greedy tail.
//
// applePrefix is what macOS puts in front of all of that once stderr is no longer a terminal - which is exactly the
// situation the capture creates:
//
//	2026-08-29 14:27:18.455 cli[22667:2454969] 2026-08-29 14:27:18.451476 [W:onnxruntime:, ...] message
//
// It is a second timestamp plus the process name, pid and thread id, all of which slog already records or does not
// need, so it is matched only to be discarded. Without this the lines parse as unrecognized output and lose their
// severity, which is how the omission shows up.
const applePrefix = `(?:\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+ \S+\[\d+:[0-9a-fA-Fx]+\] )?`

var ortLineRe = regexp.MustCompile(
	`^` + applePrefix +
		`\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+ \[([VIWEF]):([^:]*):([^,]*), ([^\]]*)\] (.*)$`)

// ortEnvName is the environment name the Go binding hands to CreateEnv. It appears as the logger id on every record
// that is not tied to a session, which makes it noise rather than information.
const ortEnvName = "Golang onnxruntime environment"

// maxOrtAttrs is how many elements parseOrtLine's attribute slice can reach: six key/value pairs, of which the first
// four are on nearly every line. Sizing it once saves the slice regrowing 2 -> 4 -> 8 on each record.
const maxOrtAttrs = 12

// ortRecord is one parsed ONNX Runtime log line, in the shape slog wants it.
type ortRecord struct {
	level slog.Level
	msg   string
	attrs []any
}

// parseOrtLine turns a line of ONNX Runtime log output into a record, reporting false for anything that is not in
// ORT's format - other libraries writing to the same descriptor, or the continuation lines of a multi-line message.
func parseOrtLine(line string) (ortRecord, bool) {
	m := ortLineRe.FindStringSubmatch(line)
	if m == nil {
		return ortRecord{}, false
	}

	rec := ortRecord{level: ortSeverity(m[1]), msg: m[5], attrs: make([]any, 0, maxOrtAttrs)}
	rec.attrs = append(rec.attrs, "source", "onnxruntime")

	// m[4] is ORT's CodeLocation: "file:line function". A few call sites omit the function, and some builds carry a
	// full source path instead of a bare file name, so neither half can be taken for granted.
	loc, fn, _ := strings.Cut(m[4], " ")
	if file, num, ok := strings.Cut(loc, ":"); ok {
		rec.attrs = append(rec.attrs, "ort_file", path.Base(file))

		if n, err := strconv.Atoi(num); err == nil {
			rec.attrs = append(rec.attrs, "ort_line", n)
		}
	}

	if fn != "" {
		rec.attrs = append(rec.attrs, "ort_func", fn)
	}

	// Worth recording only when it identifies a session; the environment name is on nearly every line.
	if id := strings.TrimSpace(m[3]); id != "" && id != ortEnvName {
		rec.attrs = append(rec.attrs, "ort_logger", id)
	}

	// slog has no level above ERROR, so fatal would otherwise be indistinguishable from an ordinary error.
	if m[1] == "F" {
		rec.attrs = append(rec.attrs, "ort_fatal", true)
	}

	return rec, true
}

// ortSeverity maps ORT's severity letter onto a slog level. The category (always "onnxruntime") is deliberately
// dropped: a constant attribute on every single record is noise.
func ortSeverity(letter string) slog.Level {
	switch letter {
	case "V":
		return slog.LevelDebug
	case "I":
		return slog.LevelInfo
	case "W":
		return slog.LevelWarn
	default: // "E" and "F"
		return slog.LevelError
	}
}
