package utils

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
)

// modelDataFile caches the last manifest that was fetched successfully, so a slow network degrades to stale-but-signed
// rather than to no verification at all.
const modelDataFile = ".models.json"

// modelDataUrl lists the models directory of the Hugging Face repository. The limit is not optional: the tree endpoint
// pages at 50 by default, and silently returning the first page would drop models off the manifest - which reads as
// "this model has no known hash" and installs it unverified.
const modelDataUrl = "https://huggingface.co/api/models/vegidio/open-photo-ai/tree/main/models?limit=1000"

// modelDataUrlOverride redirects the fetch in tests, so the cache and fallback behaviour can be exercised without
// reaching Hugging Face. It is empty everywhere else.
var modelDataUrlOverride string

type huggingFaceFile struct {
	Path string `json:"path"`
	Size int64  `json:"size"`
	LFS  struct {
		OID string `json:"oid"`
	} `json:"lfs"`
}

// LoadModelData returns the manifest of downloadable models - name, size and hash - preferring a fresh copy from Hugging
// Face and falling back to the last one that was fetched successfully.
//
// The fallback is what keeps model verification honest. The request is on the startup path and so is deliberately
// short-lived, and without a cache a two-second timeout on a slow connection turned every expected hash into an empty
// string: the whole session then downloaded models with nothing checking them, and said nothing about it.
//
// The hash is the Git LFS object ID, which for LFS is the SHA-256 of the file contents, so it can be compared directly
// against a hash computed over the download.
func LoadModelData() ([]internal.RemoteModelData, error) {
	data, err := fetchModelData()
	if err == nil {
		if saveErr := saveModelData(data); saveErr != nil {
			internal.Log().Warn("failed to cache the model manifest", "err", saveErr)
		}

		internal.Log().Debug("loaded remote model data", "count", len(data))
		return data, nil
	}

	internal.Log().Warn("model manifest fetch failed; falling back to the cached copy", "err", err)

	cached, cacheErr := readModelData()
	if cacheErr != nil {
		return nil, errors.CombineErrors(err, cacheErr)
	}

	internal.Log().Info("using the cached model manifest", "count", len(cached))
	return cached, nil
}

// region - Private functions

func fetchModelData() ([]internal.RemoteModelData, error) {
	// Bounded because this runs before the UI is usable. A failure is no longer costly, so the budget is generous
	// enough to survive a slow handshake without being long enough to feel like a hang.
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	url := modelDataUrl
	if modelDataUrlOverride != "" {
		url = modelDataUrlOverride
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to create request to %s", url)
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to send request to %s", url)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, errors.Newf("bad status code: %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, errors.Wrap(err, "failed to read response body")
	}

	var files []huggingFaceFile
	if err = json.Unmarshal(body, &files); err != nil {
		return nil, errors.Wrap(err, "failed to unmarshal JSON")
	}

	return lo.Map(files, func(file huggingFaceFile, _ int) internal.RemoteModelData {
		return internal.RemoteModelData{
			Name: filepath.Base(file.Path),
			Size: file.Size,
			Hash: file.LFS.OID,
		}
	}), nil
}

// saveModelData writes the manifest atomically, so a fallback never reads a half-written cache.
func saveModelData(data []internal.RemoteModelData) error {
	dir, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return errors.Wrap(err, "failed to resolve the models directory")
	}

	return internal.WriteJSONAtomic(dir, modelDataFile, data)
}

func readModelData() ([]internal.RemoteModelData, error) {
	dir, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return nil, errors.Wrap(err, "failed to resolve the models directory")
	}

	encoded, err := os.ReadFile(filepath.Join(dir, modelDataFile))
	if err != nil {
		return nil, errors.Wrap(err, "failed to read the cached model manifest")
	}

	var data []internal.RemoteModelData
	if err = json.Unmarshal(encoded, &data); err != nil {
		return nil, errors.Wrap(err, "failed to decode the cached model manifest")
	}

	if len(data) == 0 {
		return nil, errors.New("the cached model manifest is empty")
	}

	return data, nil
}

// endregion
