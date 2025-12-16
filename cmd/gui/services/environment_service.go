package services

import "runtime"

type EnvironmentService struct{}

func (s *EnvironmentService) GetOS() string {
	return runtime.GOOS
}

func (s *EnvironmentService) GetArch() string {
	return runtime.GOARCH
}
