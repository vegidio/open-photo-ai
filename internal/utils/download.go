package utils

import (
	"context"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// downloadClient has bounded connect + header timeouts so a stalled server can't hang Initialize
// indefinitely. Body reads are left unbounded because model zips can be hundreds of MB on slow links.
var downloadClient = &http.Client{
	Transport: &http.Transport{
		DialContext: (&net.Dialer{
			Timeout:   30 * time.Second,
			KeepAlive: 30 * time.Second,
		}).DialContext,
		TLSHandshakeTimeout:   30 * time.Second,
		ResponseHeaderTimeout: 30 * time.Second,
		IdleConnTimeout:       90 * time.Second,
		ExpectContinueTimeout: 5 * time.Second,
	},
}

// PrepareDependency downloads a file into destination - a subdirectory of the user's config directory - unless
// fileCheck says a correct copy is already there. A `.7z` download is extracted in place and the archive removed.
func PrepareDependency(
	ctx context.Context,
	url, destination string,
	fileCheck *types.FileCheck,
	onProgress types.DownloadProgress,
) error {
	if !shouldDownload(destination, fileCheck) {
		internal.Log().Debug("dependency present, skipping download", "destination", destination)
		return nil
	}

	internal.Log().Info("downloading dependency", "url", url, "destination", destination)

	fileName := filepath.Base(url)
	file, err := fs.MkUserConfigFile(internal.AppName, destination, fileName)
	if err != nil {
		return errors.Wrap(err, "failed to create config file")
	}
	defer file.Close()

	err = downloadFile(ctx, url, file, onProgress)
	if err != nil {
		return errors.Wrap(err, "failed to [download] dependency")
	}

	ext := filepath.Ext(fileName)

	if ext == ".7z" {
		// Close the file before unzipping it on Windows
		file.Close()
		defer os.Remove(file.Name())

		internal.Log().Info("extracting archive", "file", fileName)

		targetDir := filepath.Dir(file.Name())
		err = fs.Un7zip(file.Name(), targetDir)
		if err != nil {
			return errors.Wrap(err, "failed to unzip dependency")
		}
	}

	// Verify the freshly downloaded/extracted artifact so corruption is caught now, not on the next
	// launch (shouldDownload only hashes a pre-existing file). A mismatch removes the bad artifact.
	if err = verifyDownload(destination, fileCheck); err != nil {
		return err
	}

	internal.Log().Info("dependency ready", "url", url)
	return nil
}

// region - Progress reader

// progressInterval is the shortest gap between two progress callbacks. io.Copy reads in 32 KB chunks, so reporting on
// every Read means ~16,000 callbacks for a 500 MB dependency — each one crossing into the GUI's event bus and on to the
// frontend. Throttling keeps the reporting smooth without making the download drive the UI.
const progressInterval = 100 * time.Millisecond

type progressReader struct {
	reader     io.Reader
	total      int64
	downloaded int64
	onProgress types.DownloadProgress
	lastReport time.Time
}

func (pr *progressReader) Read(p []byte) (int, error) {
	n, err := pr.reader.Read(p)
	pr.downloaded += int64(n)

	if pr.onProgress != nil {
		// The final callback is always emitted, so the UI lands on 100% (or on the true total for a failed transfer)
		// rather than wherever the last tick happened to fall.
		done := err != nil
		if done || time.Since(pr.lastReport) >= progressInterval {
			pr.lastReport = time.Now()

			percent := 0.0
			if pr.total > 0 {
				percent = float64(pr.downloaded) / float64(pr.total)
			}

			pr.onProgress(pr.downloaded, pr.total, percent)
		}
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
	if !fs.FileExists(filePath) {
		return true
	}

	// Debug override (opai.SetSkipModelVerification): an existing model is used as-is, so a different
	// hash or an empty file no longer forces a re-download. Scoped to model files only.
	if destination == "models" && internal.SkipModelVerification() {
		return false
	}

	hash, err := crypto.Sha256File(filePath)
	if err != nil {
		return true
	}

	return fileCheck.Hash != "" && fileCheck.Hash != hash
}

func downloadFile(ctx context.Context, url string, dstFile *os.File, onProgress types.DownloadProgress) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return errors.Wrap(err, "failed to build request")
	}

	resp, err := downloadClient.Do(req)
	if err != nil {
		return errors.Wrap(err, "failed to download file")
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return errors.Newf("bad status: %s", resp.Status)
	}

	totalSize := resp.ContentLength

	reader := &progressReader{
		reader:     resp.Body,
		total:      totalSize,
		onProgress: onProgress,
	}

	_, err = io.Copy(dstFile, reader)
	if err != nil {
		return errors.Wrap(err, "failed to write file")
	}

	return nil
}

// verifyDownload re-hashes the checked file after a download/extract and fails (deleting the bad
// artifact) if it doesn't match the expected hash. It is a no-op when there's nothing to verify.
func verifyDownload(destination string, fileCheck *types.FileCheck) error {
	if fileCheck == nil || fileCheck.Hash == "" {
		return nil
	}

	configDir, err := fs.MkUserConfigDir(internal.AppName, destination)
	if err != nil {
		return errors.Wrap(err, "failed to resolve config dir for verification")
	}

	filePath := filepath.Join(configDir, fileCheck.Path)
	hash, err := crypto.Sha256File(filePath)
	if err != nil {
		return errors.Wrap(err, "failed to hash downloaded file")
	}

	if hash != fileCheck.Hash {
		internal.Log().Warn("download hash mismatch; removing artifact",
			"file", fileCheck.Path, "expected", fileCheck.Hash, "got", hash)

		if rmErr := os.Remove(filePath); rmErr != nil {
			internal.Log().Warn("failed to remove corrupt download", "path", filePath, "err", rmErr)
		}

		return errors.Newf("hash mismatch for %s: expected %s, got %s", fileCheck.Path, fileCheck.Hash, hash)
	}

	return nil
}

// endregion
