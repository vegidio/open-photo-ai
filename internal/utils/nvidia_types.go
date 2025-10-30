package utils

import (
	"encoding/json"
)

type NvidiaCuda struct {
	Cublas  Asset `json:"libcublas"`
	Cufft   Asset `json:"libcufft"`
	Runtime Asset `json:"cuda_cudart"`
}

type NvidiaCudnn struct {
	Cudnn Asset `json:"cudnn"`
}

func (n *NvidiaCudnn) UnmarshalJSON(data []byte) error {
	var aux struct {
		Cudnn struct {
			Name       string `json:"name"`
			License    string `json:"license_path"`
			LinuxAmd64 struct {
				Version File `json:"cuda12"`
			} `json:"linux-x86_64"`
			LinuxArm64 struct {
				Version File `json:"cuda12"`
			} `json:"linux-aarch64"`
			WindowsAmd64 struct {
				Version File `json:"cuda12"`
			} `json:"windows-x86_64"`
		} `json:"cudnn"`
	}

	if err := json.Unmarshal(data, &aux); err != nil {
		return err
	}

	n.Cudnn.Name = aux.Cudnn.Name
	n.Cudnn.License = aux.Cudnn.License
	n.Cudnn.LinuxAmd64 = aux.Cudnn.LinuxAmd64.Version
	n.Cudnn.LinuxArm64 = aux.Cudnn.LinuxArm64.Version
	n.Cudnn.WindowsAmd64 = aux.Cudnn.WindowsAmd64.Version

	return nil
}

type Asset struct {
	Name         string `json:"name"`
	License      string `json:"license_path"`
	LinuxAmd64   File   `json:"linux-x86_64"`
	LinuxArm64   File   `json:"linux-aarch64"`
	WindowsAmd64 File   `json:"windows-x86_64"`
}

type File struct {
	Url string `json:"relative_path"`
}
