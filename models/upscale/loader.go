package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// FormatUpscaleName builds the display name used by every convolutional upscale variant. Upscale is the one family
// whose name carries the scale, so it composes that here and leaves the precision suffix to the shared formatter.
func FormatUpscaleName(label string, scale float64, precision types.Precision) string {
	return utils.FormatModelName(fmt.Sprintf("%s %.4gx", label, scale), precision)
}
