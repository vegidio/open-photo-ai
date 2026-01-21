package utils

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"

	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// PrepareDependency downloads a file from the given URL to the destination directory if it doesn't already exist.
//
// If the downloaded file is a zip archive, it will be automatically extracted to the destination directory and the zip
// file will be removed.
//
// # Parameters:
//   - url: the URL to download the file from.
//   - destination: the subdirectory within the user's config directory to store the file.
//   - checkFile: the file to check for existence and correctness.
//   - onProgress: optional callback function to track download progress.
//
// Returns an error if the download or extraction fails, nil otherwise.
func PrepareDependency(
	url, destination string,
	fileCheck *types.FileCheck,
	onProgress types.DownloadProgress,
) error {
	if !shouldDownload(destination, fileCheck) {
		return nil
	}

	fileName := filepath.Base(url)
	file, err := fs.MkUserConfigFile(internal.AppName, destination, fileName)
	if err != nil {
		return err
	}
	defer file.Close()

	err = downloadFile(url, file, onProgress)
	if err != nil {
		return fmt.Errorf("[download] %w", err)
	}

	ext := filepath.Ext(fileName)

	// If it's a zip file, unzip it
	if ext == ".zip" {
		// Close the file before unzipping it on Windows
		file.Close()
		defer os.Remove(file.Name())

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
	onProgress types.DownloadProgress
}

func (pr *progressReader) Read(p []byte) (int, error) {
	n, err := pr.reader.Read(p)
	pr.downloaded += int64(n)

	if pr.onProgress != nil {
		percent := 0.0
		if pr.total > 0 {
			percent = float64(pr.downloaded) / float64(pr.total)
		}

		pr.onProgress(pr.downloaded, pr.total, percent)
	}

	return n, err
}

// endregion

// region - Private functions

func shouldDownload(destination string, fileCheck *types.FileCheck) bool {
	if fileCheck == nil {
		return true
	}

	configDir, err := fs.MkUserConfigDir(internal.AppName, destination)
	if err != nil {
		return true
	}

	filePath := filepath.Join(configDir, fileCheck.Path)
	if _, err = os.Stat(filePath); os.IsNotExist(err) {
		return true
	}

	hash, err := crypto.Sha256File(filePath)
	if err != nil {
		return true
	}

	return fileCheck.Hash != "" && fileCheck.Hash != hash
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
