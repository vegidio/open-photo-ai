package services

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

type OsService struct{}

func (o *OsService) RevealInFileManager(path string) error {
	if path == "" {
		return fmt.Errorf("empty path")
	}

	abs, err := filepath.Abs(path)
	if err != nil {
		return err
	}

	// If the path doesn't exist, at least open the parent directory.
	fi, statErr := os.Stat(abs)
	if statErr != nil {
		abs = filepath.Dir(abs)
		fi = nil
	}

	switch runtime.GOOS {
	case "darwin":
		// Finder: reveal file, or open folder
		if fi != nil && fi.IsDir() {
			return exec.Command("open", abs).Run()
		}

		// "open -R <file>" reveals the file in Finder
		return exec.Command("open", "-R", abs).Run()

	case "windows":
		// Explorer: select file, or open folder
		if fi != nil && fi.IsDir() {
			return exec.Command("explorer.exe", abs).Start()
		}

		// explorer.exe /select,"C:\path\to\file"
		return exec.Command("explorer.exe", "/select,"+abs).Start()

	default:
		// linux and other unix
		// Most reliable general behavior: open the containing folder.
		dir := abs
		if fi == nil || !fi.IsDir() {
			dir = filepath.Dir(abs)
		}

		// Try to select the file in common file managers; fall back to opening the folder.
		if fi == nil || !fi.IsDir() {
			candidates := [][]string{
				{"nautilus", "--select", abs},
				{"dolphin", "--select", abs},
				{"thunar", abs},
				{"nemo", abs},
				{"pcmanfm", abs},
			}

			for _, cmd := range candidates {
				if err = exec.Command(cmd[0], cmd[1:]...).Start(); err == nil {
					return nil
				}
			}
		}

		// Fallback: open the folder with the desktop default
		return exec.Command("xdg-open", dir).Start()
	}
}

func (o *OsService) RevealDirInFileManager(path string) error {
	if path == "" {
		return fmt.Errorf("empty path")
	}

	abs, err := filepath.Abs(path)
	if err != nil {
		return err
	}

	switch runtime.GOOS {
	case "darwin":
		return exec.Command("open", abs).Run()
	case "windows":
		return exec.Command("explorer.exe", abs).Start()
	default:
		return exec.Command("xdg-open", abs).Start()
	}
}
