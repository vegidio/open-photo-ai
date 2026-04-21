package utils

import (
	"strconv"
	"strings"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/saitama"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

func IdsToOperations(opIds []string) ([]types.Operation, error) {
	operations := make([]types.Operation, 0, len(opIds))

	for _, opId := range opIds {
		values := strings.Split(opId, "_")
		if len(values) < 3 {
			return nil, errors.Errorf("invalid operation ID: %q", opId)
		}

		name := values[1]

		switch name {
		// Face Recovery
		case "athens":
			operations = append(operations, athens.Op(types.Precision(values[2])))
		case "santorini":
			operations = append(operations, santorini.Op(types.Precision(values[2])))

		// Light Adjustment
		case "paris":
			if len(values) < 4 {
				return nil, errors.Errorf("invalid operation ID: %q", opId)
			}
			intensity, err := strconv.ParseFloat(values[2], 32)
			if err != nil {
				return nil, errors.Wrapf(err, "invalid intensity in %q", opId)
			}
			operations = append(operations, paris.Op(float32(intensity), types.Precision(values[3])))

		// Upscale
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
