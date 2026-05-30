package shared

import (
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"strings"

	"github.com/DeRuina/timberjack"
	"github.com/vegidio/go-sak/fs"
	opai "github.com/vegidio/open-photo-ai"
)

// SetupLogging configures file-based structured logging for an application using the OPAI library.
//
// Logs are written in human-readable text to <config dir>/logs/opai.log, rotated daily at midnight, gzip-compressed,
// keeping at most 7 days of history. It wires both sinks at once:
//   - the opai library logger (via opai.SetLogger), and
//   - the process-wide slog default (via slog.SetDefault), used by the application's own code.
//
// The default level is INFO; set OPAI_LOG_LEVEL=debug|info|warn|error to override. The returned io.Closer must be
// closed on shutdown (defer c.Close()) to flush and stop the rotation worker.
func SetupLogging(appName string) (io.Closer, error) {
	logsDir, err := fs.MkUserConfigDir(appName, "logs")
	if err != nil {
		return nil, err
	}

	writer := &timberjack.Logger{
		Filename:    filepath.Join(logsDir, "opai.log"),
		MaxBackups:  7,
		MaxAge:      7, // days
		LocalTime:   true,
		Compression: "gzip",
		RotateAt:    []string{"00:00"}, // rotate daily at midnight
		// BackupTimeFormat is left at timberjack's default (2006-01-02T15-04-05.000), which it
		// requires to be a round-trippable layout; a date-only format is rejected at runtime.
	}

	// Mark the start of a new session with a divider so consecutive runs are easy to tell apart in
	// the append-only log file. Written raw (not via slog) to keep it a clean separator line.
	_, _ = writer.Write([]byte("---\n"))

	logger := slog.New(slog.NewTextHandler(writer, &slog.HandlerOptions{
		Level: ResolveLogLevel(slog.LevelInfo),
	}))

	opai.SetLogger(logger)  // activate the library logger
	slog.SetDefault(logger) // route the app's package-level slog to the same file

	return writer, nil
}

// ResolveLogLevel maps the OPAI_LOG_LEVEL environment variable (debug|info|warn|error) to a slog level. When the
// variable is unset or unrecognized, it returns def, letting each sink pick its own default (e.g. INFO for the log
// file, ERROR for the Wails console).
func ResolveLogLevel(def slog.Level) slog.Level {
	switch strings.ToLower(os.Getenv("OPAI_LOG_LEVEL")) {
	case "debug":
		return slog.LevelDebug
	case "info":
		return slog.LevelInfo
	case "warn", "warning":
		return slog.LevelWarn
	case "error":
		return slog.LevelError
	default:
		return def
	}
}
