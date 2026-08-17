package sharpen

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
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
) (*utils.Session, error) {
	return utils.LoadSingleSession(ctx, "sh", variant, precision, ep, onProgress)
}

// FormatSharpenName builds the display name used by every sharpen variant.
func FormatSharpenName(precision types.Precision) string {
	return utils.FormatModelName("Sharpen", precision)
}
