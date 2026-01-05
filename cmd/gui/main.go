package main

import (
	"embed"
	_ "embed"
	"gui/services"
	"gui/utils"
	"log"
	"log/slog"
	"shared"

	"github.com/vegidio/go-sak/o11y"
	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
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
	appService := services.NewAppService(app, tel)
	defer appService.Destroy()

	imageService, err := services.NewImageService(app)
	if err != nil {
		tel.LogError("Error initializing ImageService", nil, err)
		log.Fatal(err)
	}
	defer imageService.Destroy()

	dialogService := services.NewDialogService(app)
	osService := services.NewOsService(app)

	app.RegisterService(application.NewService(appService))
	app.RegisterService(application.NewService(imageService))
	app.RegisterService(application.NewService(&services.EnvironmentService{}))
	app.RegisterService(application.NewService(osService))
	app.RegisterService(application.NewService(dialogService))

	// Run the application. This blocks until the application exists
	err = app.Run()

	// If an error occurred while running the application, log it and exit.
	if err != nil {
		tel.LogError("Error running the app", nil, err)
		log.Fatal(err)
	}
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
