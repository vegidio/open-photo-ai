package utils

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"

	"github.com/vegidio/go-sak/fs"
)

func PrepareDependency(appName, url, destination, fileName string, onDownload func()) error {
	if !shouldDownload(appName, destination, fileName) {
		return nil
	}

	// Notify the user that a download is necessary
	if onDownload != nil {
		onDownload()
	}

	file, err := fs.MkUserConfigFile(appName, destination, fileName)
	if err != nil {
		return err
	}
	defer file.Close()

	err = downloadFile(url, file)
	if err != nil {
		return err
	}

	ext := filepath.Ext(fileName)

	// If it's a zip file, unzip it
	if ext == ".zip" {
		defer os.Remove(file.Name())

		targetDir := filepath.Dir(file.Name())
		err = fs.Unzip(file.Name(), targetDir)
		if err != nil {
			return err
		}
	}

	return nil
}

// region - Private functions

func shouldDownload(appName, destination, fileName string) bool {
	configDir, err := fs.MkUserConfigDir(appName, destination)
	if err != nil {
		return true
	}

	filePath := filepath.Join(configDir, fileName)
	_, fErr := os.Stat(filePath)
	return os.IsNotExist(fErr)
}

func downloadFile(url string, dstFile *os.File) error {
	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	_, err = io.Copy(dstFile, resp.Body)
	if err != nil {
		return err
	}

	return nil
}

// endregion
