package utils

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fetch"
	"github.com/vegidio/go-sak/fs"
)

const (
	cubaBaseUrl    = "https://developer.download.nvidia.com/compute/cuda/redist/"
	cudaRedistrib  = "redistrib_12.9.1.json"
	cudnnBaseUrl   = "https://developer.download.nvidia.com/compute/cudnn/redist/"
	cudnnRedistrib = "redistrib_9.14.0.json"
)

var f = fetch.New(nil, 2)

func InstallCuda() error {
	var cuda NvidiaCuda

	resp, err := f.GetResult(cubaBaseUrl+cudaRedistrib, nil, &cuda)
	if err != nil {
		return err
	} else if resp.IsError() {
		return fmt.Errorf("%s", resp.Error())
	}

	cuda.baseUrl = cubaBaseUrl
	if err = downloadLibs("open-photo-ai", "cuda", cuda.Assets()); err != nil {
		return err
	}

	return nil
}

func InstallCudnn() error {
	var cudnn NvidiaCudnn

	resp, err := f.GetResult(cudnnBaseUrl+cudnnRedistrib, nil, &cudnn)
	if err != nil {
		return err
	} else if resp.IsError() {
		return fmt.Errorf("%s", resp.Error())
	}

	cudnn.baseUrl = cudnnBaseUrl
	if err = downloadLibs("open-photo-ai", "cudnn", cudnn.Assets()); err != nil {
		return err
	}

	return nil
}

func setNvidiaPaths() error {
	configDir, err := fs.MkUserConfigDir("open-photo-ai", "libs")
	if err != nil {
		return err
	}

	// Add the application's config directory to the system PATH
	cudaPath := filepath.Join(configDir, "libs", "cuda")
	cudnnPath := filepath.Join(configDir, "libs", "cudnn")
	newPath := os.Getenv("PATH") + string(os.PathListSeparator) + cudaPath + string(os.PathListSeparator) + cudnnPath
	os.Setenv("PATH", newPath)

	return nil
}

func downloadLibs(appName, product string, assets []Asset) error {
	configDir, err := fs.MkUserConfigDir(appName, "libs", product)
	if err != nil {
		return err
	}

	requests := lo.FilterMap(assets, func(a Asset, index int) (*fetch.Request, bool) {
		downloadUrl := ""

		switch {
		case runtime.GOOS == "windows" && runtime.GOARCH == "amd64":
			downloadUrl = a.WindowsAmd64.Url
		case runtime.GOOS == "linux" && runtime.GOARCH == "amd64":
			downloadUrl = a.LinuxAmd64.Url
		case runtime.GOOS == "linux" && runtime.GOARCH == "arm64":
			downloadUrl = a.LinuxArm64.Url
		default:
			downloadUrl = a.LinuxAmd64.Url
		}

		destPath := filepath.Join(configDir, filepath.Base(downloadUrl))
		req, err := f.NewRequest(downloadUrl, destPath, nil)
		if err != nil {
			return nil, false
		}

		return req, true
	})

	ch, _ := f.DownloadFiles(requests, 3)
	for resp := range ch {
		if err = resp.Error(); err != nil {
			return err
		}

		err = extractFiles(resp.Request.FilePath, configDir)
		if err != nil {
			return err
		}
	}

	return nil
}

func extractFiles(archive, destination string) error {
	defer os.Remove(archive)

	var err error
	ext := filepath.Ext(archive)

	if ext == ".zip" {
		err = fs.Unzip(archive, destination)
	} else if ext == ".xz" {
		err = fs.UntarXz(archive, destination)
	}

	if err != nil {
		return err
	}

	libPath := filepath.Join(strings.TrimSuffix(archive, ext), "bin")
	defer os.RemoveAll(libPath)

	if err = fs.MoveFiles([]string{libPath}, destination, 0, []string{"dll", "so"}); err != nil {
		return err
	}

	return nil
}
