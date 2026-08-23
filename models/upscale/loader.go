package upscale

import (
	"context"
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
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
) (utils.Sessions, error) {
	specs := make([]utils.SessionSpec, 0, len(scales))
	for _, scale := range scales {
		specs = append(specs, utils.ModelSpec(fmt.Sprintf("up_%s_%.4gx_%s", variant, float64(scale), precision)))
	}

	return utils.LoadSessions(ctx, specs, ep, utils.EPProfile{}, onProgress)
}

// FormatUpscaleName builds the display name used by every convolutional upscale variant. Upscale is the one family
// whose name carries the scale, so it composes that here and leaves the precision suffix to the shared formatter.
func FormatUpscaleName(label string, scale float64, precision types.Precision) string {
	return utils.FormatModelName(fmt.Sprintf("%s %.4gx", label, scale), precision)
}
