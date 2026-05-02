package utils

import (
	"strconv"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/colorbalance/rio"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
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
func IdsToOperations(opIds []string) ([]types.Operation, error) {
	operations := make([]types.Operation, 0, len(opIds))

	for _, opId := range opIds {
		values := strings.Split(opId, "_")
		if len(values) < 3 {
			return nil, errors.Errorf("invalid operation ID: %q", opId)
		}

		name := values[1]

		switch name {
		// Face Recovery — "_<name>_<precision>"
		case "athens":
			operations = append(operations, athens.Op(types.Precision(values[2])))
		case "santorini":
			operations = append(operations, santorini.Op(types.Precision(values[2])))

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
