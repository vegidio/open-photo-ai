package main

import (
	"embed"
	_ "embed"
	"fmt"
	"gui/services"
	"log"
	"log/slog"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/wailsapp/wails/v3/pkg/application"
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
		Description: "A demo of using raw HTML & CSS",
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Mac: application.MacOptions{
			ApplicationShouldTerminateAfterLastWindowClosed: true,
		},
		LogLevel: slog.LevelError,
	})

	// Create a new window with the necessary options.
	app.Window.NewWithOptions(application.WebviewWindowOptions{
		Title:      "Home",
		StartState: application.WindowStateMaximised,
		MinWidth:   1280,
		MinHeight:  720,
		Mac: application.MacWindow{
			InvisibleTitleBarHeight: 50,
			Backdrop:                application.MacBackdropTranslucent,
			TitleBar:                application.MacTitleBarHiddenInset,
		},
		URL: "/",
	})

	// Services
	imageService, err := services.NewImageService(app)
	if err != nil {
		log.Fatal(err)
	}
	defer imageService.Destroy()

	app.RegisterService(application.NewService(&services.EnvironmentService{}))
	app.RegisterService(application.NewService(&services.OsService{}))
	app.RegisterService(application.NewService(&services.DialogService{}))
	app.RegisterService(application.NewService(imageService))

	// Run the application. This blocks until the application has been exited.
	err = app.Run()

	// If an error occurred while running the application, log it and exit.
	if err != nil {
		log.Fatal(err)
	}
}
