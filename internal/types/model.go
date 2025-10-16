package types

type Model interface {
	Id() string
	Name() string
	IsLoaded() bool

	Load() error
	Run(operation Operation, input InputData) (output *OuputData, err error)
	Unload()
}
