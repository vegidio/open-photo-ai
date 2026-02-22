package services

import (
	"github.com/vegidio/go-sak/o11y"
	"github.com/wailsapp/wails/v3/pkg/application"
)

func RegisterServices(app *application.App, otel *o11y.Telemetry) (func(), error) {
	appService := NewAppService(app, otel)
	app.RegisterService(application.NewService(appService))

	imageService := NewImageService(app, otel)
	app.RegisterService(application.NewService(imageService))

	dialogService := NewDialogService(app, otel)
	app.RegisterService(application.NewService(dialogService))

	osService := NewOsService(app)
	app.RegisterService(application.NewService(osService))

	return func() {
		appService.destroy()
		imageService.destroy()
		dialogService.destroy()
		osService.destroy()
	}, nil
}
