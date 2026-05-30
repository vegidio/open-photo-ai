package internal

import (
	"log/slog"
	"sync/atomic"
)

// logger is the library's logging sink. It defaults to a no-op handler so that any third-party application importing
// the OPAI library produces no log output and creates no files unless it explicitly opts in via opai.SetLogger. Our own
// binaries (cmd/gui, cmd/cli) opt in through shared.SetupLogging.
var logger atomic.Pointer[slog.Logger]

func init() {
	logger.Store(slog.New(slog.DiscardHandler))
}

// SetLogger swaps the library's logger. A nil logger is ignored. Safe for concurrent use.
func SetLogger(l *slog.Logger) {
	if l != nil {
		logger.Store(l)
	}
}

// Log returns the library's current logger. Library code must log through this rather than the package-level slog
// functions, so the library stays silent by default.
func Log() *slog.Logger {
	return logger.Load()
}
