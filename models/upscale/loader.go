package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// FormatUpscaleName builds the display name used by every convolutional upscale variant. Upscale is the one family
// whose name carries the scale, so it composes that here and leaves the precision suffix to the shared formatter.
func FormatUpscaleName(label, codename string, scale float64, precision types.Precision) string {
	// Composing the scale first would hide a missing Label from the shared formatter: "" becomes " 4x", which is not
	// empty, so its codename fallback would never fire. Hand the empty label straight through and let it substitute.
	if label == "" {
		return utils.FormatModelName("", codename, precision)
	}

	return utils.FormatModelName(fmt.Sprintf("%s %.4gx", label, scale), codename, precision)
}
