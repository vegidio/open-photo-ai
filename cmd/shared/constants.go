package shared

import "github.com/vegidio/go-sak/o11y"

const (
	// Version of the application.
	Version = "<version>"

	// OtelEndpoint specifies the endpoint for OpenTelemetry tracking.
	OtelEndpoint = "<otel>"

	// OtelEnvironment specifies the environment for OpenTelemetry tracking
	OtelEnvironment = o11y.EnvDevelopment
)
