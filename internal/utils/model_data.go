package utils

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"path/filepath"
	"strings"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/samber/lo"
	"github.com/vegidio/open-photo-ai/internal"
)

type huggingFaceFile struct {
	Path string `json:"path"`
	Size int    `json:"size"`
	LFS  struct {
		OID string `json:"oid"`
	} `json:"lfs"`
}

// LoadModelData fetches the list of available AI models from the Hugging Face repository.
//
// It retrieves model metadata including file names, sizes, and LFS hashes from the vegidio/open-photo-ai repository
// under the models directory.
//
// Returns a slice of RemoteModelData structs containing model information, or an error if:
//   - The HTTP request to Hugging Face fails
//   - The response status is not 200 OK
//   - The request takes longer than 2 seconds
func LoadModelData() ([]internal.RemoteModelData, error) {
	// We limit the request to 2 seconds to avoid blocking the GUI thread for too long.
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	url := "https://huggingface.co/api/models/vegidio/open-photo-ai/tree/main/models"
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
		return nil, errors.Wrapf(err, "bad status code: %d", resp.StatusCode)
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

func GetModelHash(id string) string {
	if model, found := lo.Find(internal.ModelData, func(model internal.RemoteModelData) bool {
		return strings.HasPrefix(model.Name, id)
	}); found {
		return model.Hash
	}

	return ""
}
