package upscale

import "github.com/vegidio/open-photo-ai/internal/types"

type Upscale struct {
	id, name string
}

func (m *Upscale) Id() string {
	return m.id
}

func (m *Upscale) Name() string {
	return m.name
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model = (*Upscale)(nil)

func NewModel() *Upscale {
	return &Upscale{
		id:   "upscale",
		name: "Upscale",
	}
}

func (m *Upscale) Prepare() error {
	return nil
}

func (m *Upscale) Run(operation types.Operation, input types.InputData) (*types.OuputData, error) {
	return nil, nil
}
