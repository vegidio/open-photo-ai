package petersburg

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpShPetersburg struct {
	precision types.Precision
}

func (o OpShPetersburg) Id() string {
	return fmt.Sprintf("sh_petersburg_%s", o.precision)
}

func (o OpShPetersburg) Precision() types.Precision {
	return o.precision
}

func (o OpShPetersburg) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpShPetersburg)(nil)

func Op(precision types.Precision) OpShPetersburg {
	return OpShPetersburg{
		precision: precision,
	}
}
