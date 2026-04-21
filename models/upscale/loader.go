package upscale

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

// LoadSessions downloads and opens one ONNX session per scale factor for the given upscale variant
// (e.g. "kyoto", "tokyo", "saitama"). The ID format matches each variant's Op.Id() — `up_<variant>_<scale>x_<precision>`.
func LoadSessions(
	ctx context.Context,
	variant string,
	precision types.Precision,
	scales []int,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) ([]*ort.DynamicAdvancedSession, error) {
	sessions := make([]*ort.DynamicAdvancedSession, 0, len(scales))

	for _, scale := range scales {
		modelId := fmt.Sprintf("up_%s_%.4gx_%s", variant, float64(scale), precision)
		modelFile := modelId + ".onnx"
		url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)
		fileCheck := &types.FileCheck{
			Path: modelFile,
			Hash: utils.GetModelHash(modelId),
		}

		if err := utils.PrepareDependency(ctx, url, "models", fileCheck, onProgress); err != nil {
			return nil, errors.Wrapf(err, "failed to prepare %s model", variant)
		}

		session, err := utils.CreateSession(modelFile, []string{"input"}, []string{"output"}, ep)
		if err != nil {
			return nil, errors.Wrapf(err, "failed to create %s session", variant)
		}

		sessions = append(sessions, session)
	}

	return sessions, nil
}

// FormatUpscaleName builds the display name used by every upscale variant.
func FormatUpscaleName(scale float64, precision types.Precision) string {
	return fmt.Sprintf("Upscale %.4gx (%s)", scale, cases.Upper(language.English).String(string(precision)))
}
