package utils

import (
	"fmt"
	"runtime"

	ort "github.com/yalue/onnxruntime_go"
)

func getTensorRTEP(cachePath string, options *ort.SessionOptions) error {
	trtOptions, err := ort.NewTensorRTProviderOptions()
	if err != nil {
		return err
	}
	defer trtOptions.Destroy()

	// TODO: Review 'trt_cuda_graph_enable' in the future; it can drastically increase the performance, but it often
	//  causes crashes when re-using the same session.
	trtOptions.Update(map[string]string{
		"device_id":                      "0",
		"trt_max_workspace_size":         "4294967296",
		"trt_fp16_enable":                "0",
		"trt_int8_enable":                "0",
		"trt_engine_hw_compatible":       "1",
		"trt_cuda_graph_enable":          "0",
		"trt_builder_optimization_level": "5",
		"trt_engine_cache_enable":        "1",
		"trt_engine_cache_path":          cachePath,
	})

	return options.AppendExecutionProviderTensorRT(trtOptions)
}

func getCudaEP(_ string, options *ort.SessionOptions) error {
	cudaOptions, err := ort.NewCUDAProviderOptions()
	if err != nil {
		return err
	}
	defer cudaOptions.Destroy()

	// TODO: Review 'enable_cuda_graph' in the future; it can drastically increase the performance, but it often
	//  causes crashes when re-using the same session.
	cudaOptions.Update(map[string]string{
		"device_id":                    "0",
		"do_copy_in_default_stream":    "1",
		"cudnn_conv_algo_search":       "EXHAUSTIVE",
		"cudnn_conv_use_max_workspace": "1",
		"enable_cuda_graph":            "0",
		"gpu_mem_limit":                "0",
	})

	return options.AppendExecutionProviderCUDA(cudaOptions)
}

func getDirectMLEP(_ string, options *ort.SessionOptions) error {
	return options.AppendExecutionProviderDirectML(0)
}

func getCoreMLEP(cachePath string, options *ort.SessionOptions) error {
	return options.AppendExecutionProviderCoreMLV2(map[string]string{
		"ModelFormat":              "MLProgram",
		"MLComputeUnits":           "ALL",
		"RequireStaticInputShapes": "1",
		"EnableOnSubgraphs":        "0",
		"ModelCacheDirectory":      cachePath,
	})
}

func getOpenVINOEP(cachePath string, options *ort.SessionOptions) error {
	return options.AppendExecutionProviderOpenVINO(map[string]string{
		"device_type":    "AUTO",
		"precision":      "FP32",
		"num_of_threads": fmt.Sprintf("%d", runtime.NumCPU()),
		"num_streams":    "2",
		"cache_dir":      cachePath,
	})
}
