package utils

import (
	"context"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/google/go-github/v74/github"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/crypto"
	"github.com/vegidio/go-sak/fs"
	gitsak "github.com/vegidio/go-sak/github"
	"github.com/vegidio/go-sak/memo"
)

func PrepareModel(appName, modelName, tag string, onDownload func()) error {
	if url, yes := ShouldDownloadModel(appName, modelName, tag); yes {
		// Notify the user that the model will be downloaded
		if onDownload != nil {
			onDownload()
		}

		if err := DownloadModel(url, appName, modelName); err != nil {
			return err
		}
	}

	return nil
}

func ShouldDownloadModel(appName, modelName, tag string) (string, bool) {
	configDir, err := fs.MkUserConfigDir(appName, "models")
	if err != nil {
		log.Fatalf("error getting user config directory: %v\n", err)
	}

	url, remoteHash, err := getLatestModel(appName, modelName, tag)
	if err != nil {
		log.Fatalf("error downloading the latest model: %v\n", err)
	}

	modelPath := filepath.Join(configDir, modelName)
	if _, fErr := os.Stat(modelPath); os.IsNotExist(fErr) {
		// The model is not present, so we must download it
		return url, true
	}

	localHash, err := crypto.Sha256File(modelPath)
	if err != nil {
		log.Fatalf("error getting the model signature: %v\n", err)
	}

	return url, localHash != remoteHash
}

func DownloadModel(url, appName, modelName string) error {
	file, err := fs.MkUserConfigFile(appName, "models", modelName)
	if err != nil {
		return err
	}
	defer file.Close()

	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	_, err = io.Copy(file, resp.Body)
	if err != nil {
		return err
	}

	return nil
}

// region - Private functions

func getLatestModel(appName, modelName, tag string) (string, string, error) {
	cachePath, err := fs.MkUserConfigDir(appName, "cache", "models")
	if err != nil {
		return "", "", err
	}

	opts := memo.CacheOpts{MaxEntries: 100, MaxCapacity: 1024 * 1024}
	m, err := memo.NewDiskOnly(cachePath, opts)
	if err != nil {
		return "", "", err
	}
	defer m.Close()

	ctx := context.Background()
	key := memo.KeyFrom("getLatestModel", modelName, tag)
	ttl := time.Hour * 24 * 7 // Cache for 1 week

	release, err := memo.Do(m, ctx, key, ttl, func(ctx context.Context) (*github.RepositoryRelease, error) {
		r, gErr := gitsak.GetReleaseByName("vegidio", "open-photo-ai", tag)
		if gErr != nil {
			return nil, gErr
		}

		return r, nil
	})

	if err != nil {
		return "", "", err
	}

	asset, found := lo.Find(release.Assets, func(item *github.ReleaseAsset) bool {
		return item.GetName() == modelName
	})

	if !found {
		return "", "", fmt.Errorf("model not found in the repository")
	}

	url := asset.GetBrowserDownloadURL()
	hash := strings.TrimPrefix(asset.GetDigest(), "sha256:")

	return url, hash, nil
}

// endregion
