package malmo

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnMalmo struct {
	precision types.Precision
}

func (o OpDnMalmo) Id() string {
	return fmt.Sprintf("dn_malmo_%s", o.precision)
}

func (o OpDnMalmo) Precision() types.Precision {
	return o.precision
}

func (o OpDnMalmo) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpDnMalmo)(nil)

func Op(precision types.Precision) OpDnMalmo {
	return OpDnMalmo{
		precision: precision,
	}
}
