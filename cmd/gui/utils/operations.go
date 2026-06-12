package utils

import (
	"fmt"
	"image"
	"image/color"
	"strconv"
	"strings"

	guitypes "gui/types"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/models/colorbalance/rio"
	"github.com/vegidio/open-photo-ai/models/denoise/gothenburg"
	"github.com/vegidio/open-photo-ai/models/denoise/malmo"
	"github.com/vegidio/open-photo-ai/models/denoise/stockholm"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/sharpen/moscow"
	"github.com/vegidio/open-photo-ai/models/sharpen/novgorod"
	"github.com/vegidio/open-photo-ai/models/sharpen/petersburg"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/saitama"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

// IdsToOperations parses operation IDs into concrete model operations.
//
// Operation ID format: "<type>_<name>_<paramA>[_<paramB>]"
//
//	— e.g. "fr_athens_fp32", "la_paris_0.5_fp32", "up_tokyo_4x_fp32".
//
// The "<type>" prefix is not consumed here (selection happens by <name>);
// "<paramA>" is the scale for upscale (with a "x" suffix) or the intensity for light adjustment; the final segment is
// always the precision.
//
// params carries the pre-detected faces forwarded to the face-recovery operations (athens/santorini); other operations
// ignore it.
func IdsToOperations(opIds []string, params guitypes.InferenceParams) ([]types.Operation, error) {
	operations := make([]types.Operation, 0, len(opIds))

	for _, opId := range opIds {
		values := strings.Split(opId, "_")
		if len(values) < 3 {
			return nil, errors.Errorf("invalid operation ID: %q", opId)
		}

		name := values[1]

		switch name {
		// Face Recovery — "_<name>_<precision>" (faces are detected independently and supplied by the caller)
		case "athens":
			operations = append(operations, athens.Op(types.Precision(values[2]), params.Faces))
		case "santorini":
			operations = append(operations, santorini.Op(types.Precision(values[2]), params.Faces))

		// Light Adjustment — "_<name>_<intensity>_<precision>"
		case "paris":
			if len(values) < 4 {
				return nil, errors.Errorf("invalid operation ID: %q", opId)
			}

			intensity, err := strconv.ParseFloat(values[2], 32)
			if err != nil {
				return nil, errors.Wrapf(err, "invalid intensity in %q", opId)
			}

			operations = append(operations, paris.Op(float32(intensity), types.Precision(values[3])))

		// Denoise — "_<name>_<intensity>_<precision>" (older IDs without an intensity segment default to 1.0)
		case "stockholm", "gothenburg", "malmo":
			intensity, precision, err := parseIntensity(values)
			if err != nil {
				return nil, errors.Wrapf(err, "invalid intensity in %q", opId)
			}

			switch name {
			case "stockholm":
				operations = append(operations, stockholm.Op(intensity, precision))
			case "gothenburg":
				operations = append(operations, gothenburg.Op(intensity, precision))
			case "malmo":
				operations = append(operations, malmo.Op(intensity, precision))
			}

		// Sharpen — "_<name>_<intensity>_<precision>" (older IDs without an intensity segment default to 1.0)
		case "moscow", "petersburg", "novgorod":
			intensity, precision, err := parseIntensity(values)
			if err != nil {
				return nil, errors.Wrapf(err, "invalid intensity in %q", opId)
			}

			switch name {
			case "moscow":
				operations = append(operations, moscow.Op(intensity, precision))
			case "petersburg":
				operations = append(operations, petersburg.Op(intensity, precision))
			case "novgorod":
				operations = append(operations, novgorod.Op(intensity, precision))
			}

		// Color Balance — "_<name>_<intensity>_<precision>"
		case "rio":
			if len(values) < 4 {
				return nil, errors.Errorf("invalid operation ID: %q", opId)
			}

			intensity, err := strconv.ParseFloat(values[2], 32)
			if err != nil {
				return nil, errors.Wrapf(err, "invalid intensity in %q", opId)
			}

			operations = append(operations, rio.Op(float32(intensity), types.Precision(values[3])))

		// Upscale — "_<name>_<scale>x_<precision>"
		case "tokyo", "kyoto", "saitama":
			if len(values) < 4 {
				return nil, errors.Errorf("invalid operation ID: %q", opId)
			}

			scale, err := strconv.ParseFloat(strings.TrimSuffix(values[2], "x"), 64)
			if err != nil {
				return nil, errors.Wrapf(err, "invalid scale in %q", opId)
			}

			precision := types.Precision(values[3])

			switch name {
			case "tokyo":
				operations = append(operations, tokyo.Op(scale, precision))
			case "kyoto":
				operations = append(operations, kyoto.Op(scale, precision))
			case "saitama":
				operations = append(operations, saitama.Op(scale, precision))
			}

		default:
			return nil, errors.Errorf("unknown operation variant %q in ID %q", name, opId)
		}
	}

	return operations, nil
}

// ApplyCropInfo applies the user's flip/rotate/crop to the image in that order (flip → rotate → crop), matching how the
// Crop/Rotate modal reports its coordinates. A zero CropInfo (Width <= 0 || Height <= 0) is a no-op and returns the
// image unchanged. The rotation is negated because imaging.Rotate is counter-clockwise for positive angles while the
// frontend cropper reports clockwise rotation.
func ApplyCropInfo(img image.Image, c guitypes.CropInfo) image.Image {
	if c.Width <= 0 || c.Height <= 0 {
		return img
	}

	if c.FlipH {
		img = imaging.FlipH(img)
	}
	if c.FlipV {
		img = imaging.FlipV(img)
	}
	if c.Rotation != 0 {
		img = imaging.Rotate(img, -c.Rotation, color.Transparent)
	}

	return imaging.Crop(img, image.Rect(c.Left, c.Top, c.Left+c.Width, c.Top+c.Height))
}

// CropCacheKey returns a stable signature for a crop, used to make a cropped image a distinct input for the library's
// per-operation image cache (which keys on the input hash). It returns "" for a zero crop (Width <= 0 || Height <= 0)
// so uncropped runs keep their original hash. Mirrors the frontend's cropToken.
func CropCacheKey(c guitypes.CropInfo) string {
	if c.Width <= 0 || c.Height <= 0 {
		return ""
	}

	return fmt.Sprintf("#c%v-%t%t-%d-%d-%d-%d", c.Rotation, c.FlipH, c.FlipV, c.Left, c.Top, c.Width, c.Height)
}

// parseIntensity extracts the denoise/sharpen intensity and precision from a split operation ID. It accepts both the
// current "_<name>_<intensity>_<precision>" form and the older "_<name>_<precision>" form (which defaults the intensity
// to 1.0).
func parseIntensity(values []string) (float32, types.Precision, error) {
	if len(values) < 4 {
		return 1.0, types.Precision(values[2]), nil
	}

	intensity, err := strconv.ParseFloat(values[2], 32)
	if err != nil {
		return 0, "", err
	}

	return float32(intensity), types.Precision(values[3]), nil
}
