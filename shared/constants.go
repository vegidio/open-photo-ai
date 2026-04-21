package shared

import "github.com/vegidio/go-sak/o11y"

const (
	// AppName is used to identify the application in the config directory.
	AppName = "open-photo-ai"

	// Version of the application.
	Version = "<version>"

	// OtelEndpoint specifies the endpoint for OpenTelemetry tracking.
	OtelEndpoint = "<otel_endpoint>"

	// OtelAuth specifies the authentication token for OpenTelemetry tracking.
	OtelAuth = "<otel_auth>"

	// OtelEnvironment specifies the environment for OpenTelemetry tracking
	OtelEnvironment = o11y.EnvDevelopment
)
