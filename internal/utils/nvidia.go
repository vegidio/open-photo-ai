package utils

import (
	"fmt"

	"github.com/vegidio/go-sak/fetch"
)

const (
	cudaUrl  = "https://developer.download.nvidia.com/compute/cuda/redist/redistrib_12.9.1.json"
	cudnnUrl = "https://developer.download.nvidia.com/compute/cudnn/redist/redistrib_9.14.0.json"
)

var f = fetch.New(nil, 2)

func DownloadCuda() error {
	var cuda NvidiaCuda

	resp, err := f.GetResult(cudaUrl, nil, &cuda)
	if err != nil {
		return err
	} else if resp.IsError() {
		return fmt.Errorf("%s", resp.Error())
	}

	fmt.Println(cuda)
	return nil
}

func DownloadCudnn() error {
	var cudnn NvidiaCudnn

	resp, err := f.GetResult(cudnnUrl, nil, &cudnn)
	if err != nil {
		return err
	} else if resp.IsError() {
		return fmt.Errorf("%s", resp.Error())
	}

	fmt.Println(cudnn)
	return nil
}
