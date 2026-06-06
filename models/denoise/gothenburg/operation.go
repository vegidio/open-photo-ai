package gothenburg

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnGothenburg struct {
	precision types.Precision
}

func (o OpDnGothenburg) Id() string {
	return fmt.Sprintf("dn_gothenburg_%s", o.precision)
}

func (o OpDnGothenburg) Precision() types.Precision {
	return o.precision
}

func (o OpDnGothenburg) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpDnGothenburg)(nil)

func Op(precision types.Precision) OpDnGothenburg {
	return OpDnGothenburg{
		precision: precision,
	}
}
