package main

import (
	"fmt"

	openphotoai "github.com/vegidio/open-photo-ai"
)

func main() {
	if err := openphotoai.Initialize("openphotoai"); err != nil {
		fmt.Printf("Failed to initialize the model runtime: %v\n", err)
		return
	}

	defer openphotoai.Destroy()
}
