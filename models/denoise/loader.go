package denoise

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// LoadSession downloads and opens the ONNX session for the given denoise variant (e.g. "stockholm"). The ID format
// matches each variant's Op.Id() — `dn_<variant>_<precision>`. Denoise models have a single fixed-shape session (no
// scale matrix), so a single session is returned.
func LoadSession(
	ctx context.Context,
	variant string,
	precision types.Precision,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*utils.Session, error) {
	return utils.LoadSingleSession(ctx, "dn", variant, precision, ep, onProgress)
}

// FormatDenoiseName builds the display name used by every denoise variant.
func FormatDenoiseName(precision types.Precision) string {
	return utils.FormatModelName("Denoise", precision)
}
