package opai

import (
	"log/slog"

	"github.com/vegidio/open-photo-ai/internal"
)

// SetLogger activates structured logging for the OPAI library.
//
// By default, the library is completely silent — it logs nothing and creates no files. Pass a *slog.Logger to receive
// INFO/WARN/ERROR (and DEBUG) events from the library's internals, which is useful for debugging. Applications
// typically don't call this directly; the bundled binaries wire it up through shared.SetupLogging.
func SetLogger(l *slog.Logger) {
	internal.SetLogger(l)
}
