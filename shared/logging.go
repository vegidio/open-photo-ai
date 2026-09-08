package shared

import (
	"errors"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"sync/atomic"
	"time"

	"github.com/DeRuina/timberjack"
	"github.com/vegidio/go-sak/fs"
	opai "github.com/vegidio/open-photo-ai"
)

// SetupLogging configures file-based structured logging for an application using the OPAI library.
//
// Logs are written in human-readable text to <config dir>/logs/opai.log, rotated daily at midnight, gzip-compressed,
// keeping at most 7 days of history. It wires both sinks at once:
//   - the OPAI library logger (via opai.SetLogger), and
//   - the process-wide slog default (via slog.SetDefault), used by the application's own code.
//
// It also captures the process's native stderr, so the log lines that ONNX Runtime writes from C++ end up in the same
// file instead of the terminal.
//
// Logging is at INFO. The returned io.Closer must be closed on shutdown (defer c.Close()) to flush, stop the rotation
// worker and put stderr back.
//
// It may only be called once per process. Calling it twice would dup fd 2 while it is already the first capture's
// pipe, so the second capture would save the first pipe's write end as "the real stderr" and the two restores could no
// longer unwind to the terminal in any order - on top of overwriting the two process globals it sets. The second call
// returns an error rather than a no-op closer, because a caller that reached here believes it owns the logging setup
// and silently handing it a working-looking Closer would let it tear down the first call's capture.
func SetupLogging(appName string) (io.Closer, error) {
	if !loggingConfigured.CompareAndSwap(false, true) {
		return nil, errors.New("logging is already configured for this process")
	}

	logsDir, err := fs.MkUserConfigDir(appName, "logs")
	if err != nil {
		loggingConfigured.Store(false)
		return nil, err
	}

	logPath := filepath.Join(logsDir, "opai.log")

	writer := &timberjack.Logger{
		Filename:    logPath,
		MaxBackups:  7,
		MaxAge:      7, // days
		LocalTime:   true,
		Compression: "gzip",
		RotateAt:    []string{"00:00"}, // rotate daily at midnight
		// BackupTimeFormat is left at timberjack's default (2006-01-02T15-04-05.000), which it
		// requires to be a round-trippable layout; a date-only format is rejected at runtime.
	}

	// timberjack's RotateAt only fires while the process is alive across midnight. To bypass this limitation, we check
	// if the existing log is stale (last written on an earlier day), force a rotation now so each day starts with a
	// clean opai.log.
	if info, err := os.Stat(logPath); err == nil && info.Size() > 0 {
		now := time.Now()
		mt := info.ModTime()
		if mt.Year() != now.Year() || mt.YearDay() != now.YearDay() {
			_ = writer.RotateWithReason("startup")
		}
	}

	// Mark the start of a new session with a divider so consecutive runs are easy to tell apart in
	// the append-only log file. Written raw (not via slog) to keep it a clean separator line.
	_, _ = writer.Write([]byte("---\n"))

	logger := slog.New(slog.NewTextHandler(writer, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	}))

	opai.SetLogger(logger)  // activate the library logger
	slog.SetDefault(logger) // route the app's package-level slog to the same file

	closers := []io.Closer{writer}

	// ONNX Runtime logs from C++ straight to the process's stderr and its Go binding exposes no custom-logger hook, so
	// the descriptor is the only place its output can be intercepted. Failing to do so costs nothing but ORT's lines,
	// which is not worth failing a launch over.
	if capture, err := startStderrCapture(logger); err == nil {
		// Ahead of the writer: the capture's reader logs through it, so it has to stop first.
		closers = append([]io.Closer{capture}, closers...)
	} else {
		logger.Warn("native stderr will not be captured into the log file", "err", err)
	}

	return multiCloser(closers), nil
}

// loggingConfigured latches for the lifetime of the process, guarding the three globals SetupLogging writes:
// opai.SetLogger, slog.SetDefault, and - inside startStderrCapture - os.Stderr. That last one is only safe because it
// happens before the process has other goroutines running, a precondition a second call would break by definition.
//
// It is deliberately not cleared by Close. Close puts stderr back and stops the rotation worker; it does not undo
// slog.SetDefault, and a process that tore its logging down is shutting down rather than about to build another.
var loggingConfigured atomic.Bool

// multiCloser lets SetupLogging keep its single-io.Closer signature while owning two things that have to be torn
// down in order. Every closer runs even if an earlier one fails, so a broken stderr restore cannot leave the log
// file unflushed.
type multiCloser []io.Closer

// Close runs every closer in order and joins their errors.
func (m multiCloser) Close() error {
	errs := make([]error, 0, len(m))

	for _, c := range m {
		errs = append(errs, c.Close())
	}

	return errors.Join(errs...)
}
