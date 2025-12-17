package utils

import "os"

func AddEnvPath(path string) {
	newEnvPath := os.Getenv("PATH") + string(os.PathListSeparator) + path
	os.Setenv("PATH", newEnvPath)
}
