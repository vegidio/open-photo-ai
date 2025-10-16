package types

type Model interface {
	Id() string
	Name() string

	Prepare() error
	Run(operation Operation, input InputData) (output *OuputData, err error)
}
