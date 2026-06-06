package sharpen

import (
	"context"
	"fmt"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

// LoadSession downloads and opens the ONNX session for the given sharpen variant (e.g. "moscow"). The ID format
// matches each variant's Op.Id() — `sh_<variant>_<precision>`. Sharpen models (Restormer deblurring) have a single
// fixed-shape session (no scale matrix), so a single session is returned.
func LoadSession(
	ctx context.Context,
	variant string,
	precision types.Precision,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*ort.DynamicAdvancedSession, error) {
	modelId := fmt.Sprintf("sh_%s_%s", variant, precision)
	modelFile := modelId + ".onnx"
	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: utils.GetModelHash(modelId),
	}

	if err := utils.PrepareDependency(ctx, url, "models", fileCheck, onProgress); err != nil {
		return nil, errors.Wrapf(err, "failed to prepare %s model", variant)
	}

	internal.Log().Debug("loading model session", "model_id", modelId)

	session, err := utils.CreateSession(modelFile, []string{"input"}, []string{"output"}, ep)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to create %s session", variant)
	}

	internal.Log().Debug("model session ready", "model_id", modelId)
	return session, nil
}

// FormatSharpenName builds the display name used by every sharpen variant.
func FormatSharpenName(precision types.Precision) string {
	return fmt.Sprintf("Sharpen (%s)", cases.Upper(language.English).String(string(precision)))
}
