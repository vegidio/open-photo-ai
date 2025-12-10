package services

import "runtime"

type EnvironmentService struct{}

func (e *EnvironmentService) GetOS() string {
	return runtime.GOOS
}

func (e *EnvironmentService) GetArch() string {
	return runtime.GOARCH
}
