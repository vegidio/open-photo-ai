package athens

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

type OpFrAthens struct {
	precision types.Precision
	faces     []detection.Face
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

// CacheKey folds the selected faces into the image cache key — they are not in Id() but change the recovered output.
func (o OpFrAthens) CacheKey() string {
	return facerecovery.FacesCacheKey(o.faces)
}

var (
	_ types.Operation     = (*OpFrAthens)(nil)
	_ types.Parameterized = (*OpFrAthens)(nil)
	_ types.CacheKeyer    = (*OpFrAthens)(nil)
)

func Op(precision types.Precision, faces []detection.Face) OpFrAthens {
	return OpFrAthens{
		precision: precision,
		faces:     faces,
	}
}
