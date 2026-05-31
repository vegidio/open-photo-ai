package santorini

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

type OpFrSantorini struct {
	precision types.Precision
	faces     []facedetection.Face
}

func (o OpFrSantorini) Id() string {
	return fmt.Sprintf("fr_santorini_%s", o.precision)
}

func (o OpFrSantorini) Precision() types.Precision {
	return o.precision
}

func (o OpFrSantorini) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params exposes the pre-detected faces to Model.Run. Faces are not part of the operation's identity, so they are
// passed per-run rather than stored on the (registry-cached) model.
func (o OpFrSantorini) Params() map[string]any {
	return map[string]any{facerecovery.ParamFaces: o.faces}
}

var (
	_ types.Operation     = (*OpFrSantorini)(nil)
	_ types.Parameterized = (*OpFrSantorini)(nil)
)

func Op(precision types.Precision, faces []facedetection.Face) OpFrSantorini {
	return OpFrSantorini{
		precision: precision,
		faces:     faces,
	}
}
