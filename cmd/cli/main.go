package main

import (
	"fmt"

	"github.com/jaypipes/ghw"
)

func main() {
	//if err := opai.Initialize("open-photo-ai", nil); err != nil {
	//	fmt.Printf("Failed to initialize the AI runtime: %v\n", err)
	//	return
	//}
	//defer opai.Destroy()

	gpu, err := ghw.GPU()
	if err != nil {
		fmt.Printf("Failed to get GPU info: %v\n", err)
		return
	}

	fmt.Printf("GPU: %s\n", gpu.GraphicsCards)

	//fileName := "test"
	//
	//inputData, err := opai.LoadImage("/Users/vegidio/Desktop/" + fileName + ".jpg")
	//if err != nil {
	//	fmt.Printf("Failed to load the input image: %v\n", err)
	//	return
	//}
	//
	//ops := []types.Operation{
	//	//newyork.Op(types.PrecisionFp32),
	//	//athens.Op(types.PrecisionFp32),
	//	//santorini.Op(types.PrecisionFp32),
	//	kyoto.Op(kyoto.ModeGeneral, 4, types.PrecisionFp32),
	//	//tokyo.Op(4, types.PrecisionFp32),
	//}
	//
	//ctx := context.Background()
	//now := time.Now()
	//
	//outputData, err := opai.Process(ctx, inputData, func(name string, progress float64) {
	//	fmt.Printf("%s - Progress: %.1f%%\n", name, progress*100)
	//}, ops...)
	//
	//if err != nil {
	//	fmt.Printf("Failed to upscale the image: %v\n", err)
	//	return
	//}
	//
	//since := time.Since(now)
	//fmt.Println("Time elapsed: ", since)
	//
	//err = opai.SaveImage(&types.ImageData{
	//	FilePath: "/Users/vegidio/Desktop/" + fileName + "_new.jpg",
	//	Pixels:   outputData.Pixels,
	//}, types.FormatJpeg, 90)
	//
	//if err != nil {
	//	fmt.Printf("Failed to save the output image: %v\n", err)
	//	return
	//}
}
