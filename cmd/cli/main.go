package main

import (
	"fmt"
	"os"

	os2 "github.com/vegidio/go-sak/os"
)

func main() {
	os2.AppendEnvPath("LD_LIBRARY_PATH", "/usr/local/cuda/lib64")
	os2.AppendEnvPath("LD_LIBRARY_PATH", "/usr/local/cuda/libcoco")

	path := os.Getenv("LD_LIBRARY_PATH")
	fmt.Println(path)
}
