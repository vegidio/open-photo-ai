package services

import (
	"github.com/vegidio/go-sak/o11y"
	"github.com/wailsapp/wails/v3/pkg/application"
)

func RegisterServices(app *application.App, tel *o11y.Telemetry) (func(), error) {
	appService := NewAppService(app, tel)
	app.RegisterService(application.NewService(appService))

	imageService := NewImageService(app, tel)
	app.RegisterService(application.NewService(imageService))

	dialogService := NewDialogService(app, tel)
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
