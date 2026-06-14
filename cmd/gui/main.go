package main

import (
	"embed"
	"fmt"
	"gui/services"
	"gui/utils"
	"log"
	"log/slog"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/o11y"
	"github.com/vegidio/go-sak/os"
	"github.com/vegidio/open-photo-ai/shared"
	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
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

	// Track of system info
	shared.ReportSystemInfo(otel)

	// Create a new Wails application by providing the necessary options.
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

	// Create a new window with the necessary options.
	win := app.Window.NewWithOptions(application.WebviewWindowOptions{
		Title:      "Open Photo AI",
		StartState: application.WindowStateMaximised,
		MinWidth:   1280,
		MinHeight:  720,
		Mac: application.MacWindow{
			InvisibleTitleBarHeight: 50,
			Backdrop:                application.MacBackdropTranslucent,
			TitleBar:                application.MacTitleBarHidden,
		},
		URL:            "/",
		EnableFileDrop: true,
	})

	// Track drag and drops on the app
	eventDragAndDrop(app, win)

	// Services
	destroyServices := services.RegisterServices(app, otel)
	defer destroyServices()

	// Run the application. This blocks until the application exists
	err := app.Run()

	slog.Info("Open Photo AI exited")

	// If an error occurred while running the application, log it and exit.
	if err != nil {
		otel.LogError("Error running the app", nil, err)
		slog.Error("error running the app", "err", err)
		log.Fatalf("%+v", err)
	}
}

func setLibPathAndRestart() {
	libPaths := make([]string, 0)

	if path, err := fs.MkUserConfigDir(shared.AppName, "libs", "cuda"); err == nil {
		libPaths = append(libPaths, path)
	}
	if path, err := fs.MkUserConfigDir(shared.AppName, "libs", "cudnn"); err == nil {
		libPaths = append(libPaths, path)
	}

	slog.Info("re-executing with LD_LIBRARY_PATH", "paths", strings.Join(libPaths, ":"))
	os.ReExec(fmt.Sprintf("LD_LIBRARY_PATH=%s", strings.Join(libPaths, ":")))
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
