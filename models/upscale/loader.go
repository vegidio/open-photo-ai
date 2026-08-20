package upscale

import (
	"context"
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
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
) (utils.Sessions, error) {
	specs := make([]utils.SessionSpec, 0, len(scales))
	for _, scale := range scales {
		specs = append(specs, utils.ModelSpec(fmt.Sprintf("up_%s_%.4gx_%s", variant, float64(scale), precision)))
	}

	return utils.LoadSessions(ctx, specs, ep, utils.EPProfile{}, onProgress)
}

// FormatUpscaleName builds the display name used by every upscale variant.
func FormatUpscaleName(scale float64, precision types.Precision) string {
	return fmt.Sprintf("Upscale %.4gx (%s)", scale, cases.Upper(language.English).String(string(precision)))
}
