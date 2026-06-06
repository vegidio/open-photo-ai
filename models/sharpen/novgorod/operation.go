package novgorod

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpShNovgorod struct {
	precision types.Precision
}

func (o OpShNovgorod) Id() string {
	return fmt.Sprintf("sh_novgorod_%s", o.precision)
}

func (o OpShNovgorod) Precision() types.Precision {
	return o.precision
}

func (o OpShNovgorod) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpShNovgorod)(nil)

func Op(precision types.Precision) OpShNovgorod {
	return OpShNovgorod{
		precision: precision,
	}
}
