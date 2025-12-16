package utils

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

func PrepareDependency(url, destination, fileName string, onProgress types.DownloadProgress) error {
	if !shouldDownload(destination, fileName) {
		return nil
	}

	file, err := fs.MkUserConfigFile(internal.AppName, destination, fileName)
	if err != nil {
		return err
	}
	defer file.Close()

	err = downloadFile(url, file, onProgress)
	if err != nil {
		return err
	}

	ext := filepath.Ext(fileName)

	// If it's a zip file, unzip it
	if ext == ".zip" {
		//defer os.Remove(file.Name())

		targetDir := filepath.Dir(file.Name())
		err = fs.Unzip(file.Name(), targetDir)
		if err != nil {
			return err
		}
	}

	return nil
}

// region - Progress reader

type progressReader struct {
	reader     io.Reader
	total      int64
	downloaded int64
	onProgress func(downloaded, total int64, percent float64)
}

func (pr *progressReader) Read(p []byte) (int, error) {
	n, err := pr.reader.Read(p)
	pr.downloaded += int64(n)

	if pr.onProgress != nil {
		percent := float64(pr.downloaded) / float64(pr.total)
		pr.onProgress(pr.downloaded, pr.total, percent)
	}

	return n, err
}

// endregion

// region - Private functions

func shouldDownload(destination, fileName string) bool {
	configDir, err := fs.MkUserConfigDir(internal.AppName, destination)
	if err != nil {
		return true
	}

	filePath := filepath.Join(configDir, fileName)
	_, fErr := os.Stat(filePath)
	return os.IsNotExist(fErr)
}

func downloadFile(url string, dstFile *os.File, onProgress types.DownloadProgress) error {
	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	// Get the total file size from the Content-Length header
	totalSize := resp.ContentLength

	// Wrap the reader with a progress tracker
	reader := &progressReader{
		reader:     resp.Body,
		total:      totalSize,
		onProgress: onProgress,
	}

	_, err = io.Copy(dstFile, reader)
	if err != nil {
		return err
	}

	return nil
}

// endregion
