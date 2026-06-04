package uppsala

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

type OpDnUppsala struct {
	precision types.Precision
}

func (o OpDnUppsala) Id() string {
	return fmt.Sprintf("dn_uppsala_%s", o.precision)
}

func (o OpDnUppsala) Precision() types.Precision {
	return o.precision
}

func (o OpDnUppsala) Hash() string {
	return utils.GetModelHash(o.Id())
}

var _ types.Operation = (*OpDnUppsala)(nil)

func Op(precision types.Precision) OpDnUppsala {
	return OpDnUppsala{
		precision: precision,
	}
}
