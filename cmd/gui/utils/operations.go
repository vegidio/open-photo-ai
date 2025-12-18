package utils

import (
	"strconv"
	"strings"

	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

func IdsToOperations(opIds []string) []types.Operation {
	operations := make([]types.Operation, 0)

	for _, opId := range opIds {
		values := strings.Split(opId, "_")
		name := values[1]

		switch name {
		// Face Recovery
		case "athens":
			precision := types.Precision(values[2])
			operations = append(operations, athens.Op(precision))
		case "santorini":
			precision := types.Precision(values[2])
			operations = append(operations, santorini.Op(precision))

		// Upscale
		case "tokyo":
			scale, _ := strconv.Atoi(strings.Replace(values[2], "x", "", 1))
			precision := types.Precision(values[3])
			operations = append(operations, tokyo.Op(scale, precision))
		case "kyoto":
			mode := kyoto.Mode(values[2])
			scale, _ := strconv.Atoi(strings.Replace(values[3], "x", "", 1))
			precision := types.Precision(values[4])
			operations = append(operations, kyoto.Op(mode, scale, precision))
		}
	}

	return operations
}
