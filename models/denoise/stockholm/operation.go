package stockholm

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnStockholm struct {
	precision types.Precision
}

func (o OpDnStockholm) Id() string {
	return fmt.Sprintf("dn_stockholm_%s", o.precision)
}

func (o OpDnStockholm) Precision() types.Precision {
	return o.precision
}

func (o OpDnStockholm) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpDnStockholm)(nil)

func Op(precision types.Precision) OpDnStockholm {
	return OpDnStockholm{
		precision: precision,
	}
}
