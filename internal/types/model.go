package types

type Model interface {
	Id() string
	Name() string
	IsLoaded() bool

	Load(operation Operation) error
	Run(input InputData) (output *OutputData, err error)
	Unload()
}
