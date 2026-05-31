package athens

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

type OpFrAthens struct {
	precision types.Precision
	faces     []facedetection.Face
}

func (o OpFrAthens) Id() string {
	return fmt.Sprintf("fr_athens_%s", o.precision)
}

func (o OpFrAthens) Precision() types.Precision {
	return o.precision
}

func (o OpFrAthens) Hash() string {
	return utils.GetModelHash(o.Id())
}

// Params exposes the pre-detected faces to Model.Run. Faces are not part of the operation's identity, so they are
// passed per-run rather than stored on the (registry-cached) model.
func (o OpFrAthens) Params() map[string]any {
	return map[string]any{facerecovery.ParamFaces: o.faces}
}

var (
	_ types.Operation     = (*OpFrAthens)(nil)
	_ types.Parameterized = (*OpFrAthens)(nil)
)

func Op(precision types.Precision, faces []facedetection.Face) OpFrAthens {
	return OpFrAthens{
		precision: precision,
		faces:     faces,
	}
}
