package main

import (
	"embed"
	"fmt"
	"gui/services"
	"gui/utils"
	"log"
	"log/slog"
	stdos "os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"

	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/o11y"
	"github.com/vegidio/go-sak/os"
	"github.com/vegidio/open-photo-ai/shared"
	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

//go:embed all:frontend/dist
var assets embed.FS

const (
	minWindowWidth  = 1280
	minWindowHeight = 720
)

func main() {
	stdos.Exit(run())
}

// run holds what would otherwise be main's body so that every deferred cleanup — the log file, the OTLP exporter, and
// the Wails services — actually runs before the process exits. Calling os.Exit (or log.Fatal) directly from main skips
// deferred calls, which meant a fatal startup error was reported to telemetry and then dropped when the batching
// exporter was never flushed.
func run() int {
	// TODO: Workaround for Linux to set LD_LIBRARY_PATH; I must revisit this approach in the future
	if runtime.GOOS == "linux" {
		setLibPathAndRestart()
	}

	// Set up file-based logging (rotated daily, kept 7 days) before anything else, so all
	// downstream events — including library internals via opai.SetLogger — land in the log file.
	if logCloser, err := shared.SetupLogging(shared.AppName); err == nil {
		defer logCloser.Close()
	} else {
		log.Printf("failed to set up file logging: %v", err)
	}

	slog.Info("starting Open Photo AI", "version", shared.Version, "os", runtime.GOOS, "arch", runtime.GOARCH)

	otel := o11y.NewTelemetry(
		shared.OtelEndpoint,
		"opai",
		shared.Version,
		map[string]string{"Authorization": shared.OtelAuth},
		shared.OtelEnvironment,
		true,
	)

	defer otel.Close()

	shared.ReportSystemInfo(otel)

	app := application.New(application.Options{
		Name:        "Open Photo AI",
		Description: "An open source photo AI editor",
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Mac: application.MacOptions{
			ApplicationShouldTerminateAfterLastWindowClosed: true,
		},
		LogLevel: shared.ResolveLogLevel(slog.LevelError),
	})

	win := app.Window.NewWithOptions(application.WebviewWindowOptions{
		Title:      "Open Photo AI",
		StartState: application.WindowStateMaximised,
		Width:      minWindowWidth,
		Height:     minWindowHeight,
		MinWidth:   minWindowWidth,
		MinHeight:  minWindowHeight,
		Mac: application.MacWindow{
			InvisibleTitleBarHeight: 50,
			Backdrop:                application.MacBackdropTranslucent,
			TitleBar:                application.MacTitleBarHidden,
		},
		URL:            "/",
		EnableFileDrop: true,
	})

	maximizeOnStart(win)
	eventDragAndDrop(app, win)

	destroyServices := services.RegisterServices(app, otel)
	defer destroyServices()

	// Blocks until the application exits.
	err := app.Run()

	slog.Info("Open Photo AI exited")

	if err != nil {
		otel.LogError("Error running the app", nil, err)
		slog.Error("error running the app", "err", err)
		log.Printf("%+v", err)
		return 1
	}

	return 0
}

// setLibPathAndRestart re-executes the process with LD_LIBRARY_PATH pointing at the bundled NVIDIA libraries, because
// glibc parses that variable once at exec time: appending to it later, after the libraries have been downloaded, has no
// effect on the dlopen() calls that ONNX Runtime makes to load its execution providers.
//
// Every library that a provider links against must be listed here, even if its directory is still empty on a fresh
// install — MkUserConfigDir creates it up front so the loader doesn't mark it as non-existing and skip it for the rest
// of the process lifetime, which would break a provider downloaded later in the same session.
//
// The runtime directory is defence in depth rather than a requirement: ONNX Runtime finds its execution providers
// relative to its own location, by resolving the address of a symbol inside itself, so they are found whether or not
// the directory is on the search path. It is listed anyway because it costs nothing and because it gets the directory
// created before Initialize runs, which is the hazard described above.
func setLibPathAndRestart() {
	// ReExec sets APP_REEXEC=1 in the child and no-ops when called again, so bail out before doing any work: the child
	// would otherwise recreate the dirs and log a re-exec that never happens.
	if stdos.Getenv("APP_REEXEC") == "1" {
		return
	}

	libPaths := make([]string, 0)

	if path, err := fs.MkUserConfigDir(shared.AppName, "runtime"); err == nil {
		libPaths = append(libPaths, path)
	}
	if path, err := fs.MkUserConfigDir(shared.AppName, "libs", "cuda"); err == nil {
		libPaths = append(libPaths, path)
	}
	if path, err := fs.MkUserConfigDir(shared.AppName, "libs", "cudnn"); err == nil {
		libPaths = append(libPaths, path)
	}
	if path, err := fs.MkUserConfigDir(shared.AppName, "libs", "tensorrt"); err == nil {
		libPaths = append(libPaths, path)
	}

	// Keep any inherited value, appended last so the bundled libraries — which are version-matched to the ONNX Runtime
	// build — take precedence over a system-wide install. Merging matters: ReExec adds the variable to the inherited
	// environment rather than replacing it, so an inherited entry would otherwise compete with this one. Uniq keeps the
	// first occurrence, so the bundled dirs win and a repeated entry can't turn into a duplicated path.
	libPaths = append(libPaths, filepath.SplitList(stdos.Getenv("LD_LIBRARY_PATH"))...)
	libPaths = lo.Uniq(lo.Compact(libPaths))

	slog.Info("re-executing with LD_LIBRARY_PATH", "paths", strings.Join(libPaths, ":"))
	os.ReExec(fmt.Sprintf("LD_LIBRARY_PATH=%s", strings.Join(libPaths, ":")))
}

// maximizeOnStart maximises the window once the frontend runtime has connected, since StartState and early Maximise()
// calls are ignored by macOS before the window is shown.
//
// Maximise() also clears the size constraints, which Wails only restores on its own un-maximise, so the minimum is
// re-applied by hand once WindowDidResize shows the window has grown past it (macOS never fires WindowMaximise).
// WindowUnFullscreen needs the same fix for the same reason.
func maximizeOnStart(win *application.WebviewWindow) {
	var maximise, restore sync.Once

	win.OnWindowEvent(events.Common.WindowRuntimeReady, func(_ *application.WindowEvent) {
		maximise.Do(func() { win.Maximise() })
	})

	restoreMinSize := func(_ *application.WindowEvent) {
		win.SetMinSize(minWindowWidth, minWindowHeight)
	}

	win.OnWindowEvent(events.Common.WindowDidResize, func(event *application.WindowEvent) {
		if w, h := win.Size(); w >= minWindowWidth && h >= minWindowHeight {
			restore.Do(func() { restoreMinSize(event) })
		}
	})

	win.OnWindowEvent(events.Common.WindowUnFullscreen, restoreMinSize)
}

func eventDragAndDrop(app *application.App, win *application.WebviewWindow) {
	win.OnWindowEvent(
		events.Common.WindowFilesDropped,
		func(event *application.WindowEvent) {
			paths := event.Context().DroppedFiles()
			supported, unsupported := utils.PartitionSupportedFiles(paths)

			// Warn about and surface any unsupported files, but still load the supported ones.
			if len(unsupported) > 0 {
				slog.Warn("unsupported files dropped", "count", len(unsupported))
				showUnsupportedFilesDialog(app, unsupported)
			}

			if len(supported) == 0 {
				return
			}

			files := utils.CreateFileTypes(supported)
			slog.Info("files dropped", "count", len(files))
			app.Event.Emit(services.EventAppFilesDropped, files)
		})
}

func showUnsupportedFilesDialog(app *application.App, unsupported []string) {
	var message string
	if len(unsupported) == 1 {
		message = fmt.Sprintf("The file %q is not supported.", filepath.Base(unsupported[0]))
	} else {
		names := make([]string, len(unsupported))
		for i, path := range unsupported {
			names[i] = "  • " + filepath.Base(path)
		}
		message = "The following files are not supported:\n\n" + strings.Join(names, "\n")
	}

	dialog := app.Dialog.Error()
	dialog.SetTitle("Unsupported File(s)")
	dialog.SetMessage(message)
	dialog.Show()
}
