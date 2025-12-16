package main

import (
	"embed"
	_ "embed"
	"fmt"
	"gui/services"
	"gui/utils"
	"log"
	"log/slog"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

const AppName = "open-photo-ai"

//go:embed all:frontend/dist
var assets embed.FS

// main function serves as the application's entry point. It initializes the application, creates a window,
// and starts a goroutine that emits a time-based event every second. It subsequently runs the application and
// logs any error that might occur.
func main() {
	// Initialize the model runtime
	if err := opai.Initialize(AppName); err != nil {
		fmt.Printf("Failed to initialize the model runtime: %v\n", err)
		return
	}
	defer opai.Destroy()

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
		URL:               "/",
		EnableDragAndDrop: true,
	})

	// Track drag and drops on the app
	eventDragAndDrop(app, win)

	// Services
	imageService, err := services.NewImageService(app)
	if err != nil {
		log.Fatal(err)
	}
	defer imageService.Destroy()

	dialogService := services.NewDialogService(app)

	app.RegisterService(application.NewService(&services.EnvironmentService{}))
	app.RegisterService(application.NewService(&services.OsService{}))
	app.RegisterService(application.NewService(dialogService))
	app.RegisterService(application.NewService(imageService))

	// Run the application. This blocks until the application has been exited.
	err = app.Run()

	// If an error occurred while running the application, log it and exit.
	if err != nil {
		log.Fatal(err)
	}
}

func eventDragAndDrop(app *application.App, win *application.WebviewWindow) {
	win.OnWindowEvent(
		events.Common.WindowDropZoneFilesDropped,
		func(event *application.WindowEvent) {
			paths := event.Context().DroppedFiles()
			files := utils.CreateFileTypes(paths)

			app.Event.Emit("app:FilesDropped", files)
		})
}
