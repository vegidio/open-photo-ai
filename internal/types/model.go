package types

type Model interface {
	Id() string
	Name() string
	Run(input *InputData) (output *OutputData, err error)
	Destroy()
}
