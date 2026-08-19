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
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/sharpen/moscow"
	"github.com/vegidio/open-photo-ai/models/sharpen/novgorod"
	"github.com/vegidio/open-photo-ai/models/sharpen/petersburg"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/osaka"
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

		build, known := operationBuilders[name]
		if !known {
			return nil, errors.Errorf("unknown operation variant %q in ID %q", name, opId)
		}

		operation, err := build(values, params)
		if err != nil {
			return nil, errors.Wrapf(err, "invalid operation ID %q", opId)
		}

		operations = append(operations, operation)
	}

	return operations, nil
}

// operationBuilder turns the underscore-split segments of an operation ID into the operation it names. params carries
// the pre-detected faces that only the face-recovery models consume.
type operationBuilder func(values []string, params guitypes.InferenceParams) (types.Operation, error)

// operationBuilders maps a model name to its constructor. A table rather than a switch so that adding a model is one
// entry here, and so the shared "<name>_<amount>_<precision>" parsing is written once instead of per model.
var operationBuilders = map[string]operationBuilder{
	// Face Recovery — "_<name>_<precision>" (faces are detected independently and supplied by the caller)
	"athens":    faceRecoveryBuilder(athens.Op),
	"santorini": faceRecoveryBuilder(santorini.Op),

	// Denoise — "_<name>_<intensity>_<precision>" (older IDs without an intensity segment default to 1.0)
	"stockholm":  intensityBuilder(stockholm.Op),
	"gothenburg": intensityBuilder(gothenburg.Op),
	"malmo":      intensityBuilder(malmo.Op),

	// Sharpen — "_<name>_<intensity>_<precision>" (older IDs without an intensity segment default to 1.0)
	"moscow":     intensityBuilder(moscow.Op),
	"petersburg": intensityBuilder(petersburg.Op),
	"novgorod":   intensityBuilder(novgorod.Op),

	// Light Adjustment / Color Balance — "_<name>_<intensity>_<precision>", intensity always present
	"paris": requiredIntensityBuilder(paris.Op),
	"rio":   requiredIntensityBuilder(rio.Op),

	// Upscale — "_<name>_<scale>x_<precision>"
	"tokyo":   scaleBuilder(tokyo.Op),
	"kyoto":   scaleBuilder(kyoto.Op),
	"saitama": scaleBuilder(saitama.Op),

	// Osaka parses like the others, but its Op drops the scale from the identity: SeedVR2 restores at whatever size
	// it is handed, so one set of sessions serves every scale and the scale travels in Params instead.
	"osaka": scaleBuilder(osaka.Op),
}

// Each model's Op returns its own concrete operation type, so the builders below are generic over that type rather
// than taking a func that returns types.Operation — Go won't convert the function types implicitly.

// faceRecoveryBuilder reads "_<name>_<precision>" and forwards the caller-supplied faces.
func faceRecoveryBuilder[T types.Operation](op func(types.Precision, []detection.Face) T) operationBuilder {
	return func(values []string, params guitypes.InferenceParams) (types.Operation, error) {
		return op(types.Precision(values[2]), params.Faces), nil
	}
}

// intensityBuilder reads "_<name>_<intensity>_<precision>", tolerating the older "_<name>_<precision>" form by
// defaulting the intensity to 1.0.
func intensityBuilder[T types.Operation](op func(float32, types.Precision) T) operationBuilder {
	return func(values []string, _ guitypes.InferenceParams) (types.Operation, error) {
		intensity, precision, err := parseIntensity(values)
		if err != nil {
			return nil, errors.Wrap(err, "invalid intensity")
		}

		return op(intensity, precision), nil
	}
}

// requiredIntensityBuilder reads "_<name>_<intensity>_<precision>", where the intensity segment is mandatory.
func requiredIntensityBuilder[T types.Operation](op func(float32, types.Precision) T) operationBuilder {
	return func(values []string, _ guitypes.InferenceParams) (types.Operation, error) {
		if len(values) < 4 {
			return nil, errors.New("missing intensity segment")
		}

		intensity, err := strconv.ParseFloat(values[2], 32)
		if err != nil {
			return nil, errors.Wrap(err, "invalid intensity")
		}

		return op(float32(intensity), types.Precision(values[3])), nil
	}
}

// scaleBuilder reads "_<name>_<scale>x_<precision>".
func scaleBuilder[T types.Operation](op func(float64, types.Precision) T) operationBuilder {
	return func(values []string, _ guitypes.InferenceParams) (types.Operation, error) {
		if len(values) < 4 {
			return nil, errors.New("missing scale segment")
		}

		scale, err := strconv.ParseFloat(strings.TrimSuffix(values[2], "x"), 64)
		if err != nil {
			return nil, errors.Wrap(err, "invalid scale")
		}

		return op(scale, types.Precision(values[3])), nil
	}
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
