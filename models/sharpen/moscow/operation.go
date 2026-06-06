package moscow

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpShMoscow struct {
	precision types.Precision
}

func (o OpShMoscow) Id() string {
	return fmt.Sprintf("sh_moscow_%s", o.precision)
}

func (o OpShMoscow) Precision() types.Precision {
	return o.precision
}

func (o OpShMoscow) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpShMoscow)(nil)

func Op(precision types.Precision) OpShMoscow {
	return OpShMoscow{
		precision: precision,
	}
}
