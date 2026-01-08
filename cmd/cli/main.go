package main

import (
	"fmt"
	"os"

	os2 "github.com/vegidio/go-sak/os"
)

func main() {
	fmt.Println("Running app...")
	fmt.Println("LD_LIBRARY_PATH", os.Getenv("LD_LIBRARY_PATH"))

	os2.ReExec("LD_LIBRARY_PATH=/home/vegidio/.config/open-photo-ai/libs/cuda:/home/vegidio/.config/open-photo-ai/libs/cudnn")
}
