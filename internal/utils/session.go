package utils

import (
	"os"
	"path/filepath"

	ort "github.com/yalue/onnxruntime_go"
)

func CreateSession(appName, modelName, tag string, onDownload func()) (*ort.DynamicAdvancedSession, error) {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return nil, err
	}

	// Download the model if it's not already present
	if url, yes := ShouldDownloadModel(appName, modelName, tag); yes {
		// Notify the user that the model will be downloaded
		if onDownload != nil {
			onDownload()
		}

		if err = DownloadModel(url, appName, modelName); err != nil {
			return nil, err
		}
	}

	modelPath := filepath.Join(configDir, appName, "models", modelName)
	session, err := ort.NewDynamicAdvancedSession(modelPath, []string{"input"}, []string{"output"}, nil)
	if err != nil {
		return nil, err
	}

	return session, nil
}
