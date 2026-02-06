package main

import (
	"embed"
	_ "embed"
	"fmt"
	"gui/services"
	"gui/utils"
	"log"
	"log/slog"
	"runtime"
	"shared"
	"strings"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/go-sak/o11y"
	"github.com/vegidio/go-sak/os"
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

	tel := o11y.NewTelemetry(shared.OtelEndpoint, "opai", shared.Version, shared.OtelEnvironment, true)
	defer tel.Close()

	// Track of system info
	shared.ReportSystemInfo(tel)

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
		LogLevel: slog.LevelError,
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
	destroyServices, err := services.RegisterServices(app, tel)
	if err != nil {
		tel.LogError("Error starting the services", nil, err)
		log.Fatalf("%+v", err)
	}
	defer destroyServices()

	// Run the application. This blocks until the application exists
	err = app.Run()

	// If an error occurred while running the application, log it and exit.
	if err != nil {
		tel.LogError("Error running the app", nil, err)
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

	os.ReExec(fmt.Sprintf("LD_LIBRARY_PATH=%s", strings.Join(libPaths, ":")))
}

func eventDragAndDrop(app *application.App, win *application.WebviewWindow) {
	win.OnWindowEvent(
		events.Common.WindowFilesDropped,
		func(event *application.WindowEvent) {
			paths := event.Context().DroppedFiles()
			files := utils.CreateFileTypes(paths)

			app.Event.Emit("app:FilesDropped", files)
		})
}
