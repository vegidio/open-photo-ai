package utils

import (
	"encoding/json"

	"github.com/samber/lo"
)

type Nvidia interface {
	Assets() []Asset
}

type NvidiaCuda struct {
	baseUrl string
	Product string `json:"release_product"`
	Cublas  Asset  `json:"libcublas"`
	Cufft   Asset  `json:"libcufft"`
	Runtime Asset  `json:"cuda_cudart"`
}

func (n *NvidiaCuda) Assets() []Asset {
	assets := []Asset{n.Cublas, n.Cufft, n.Runtime}
	return lo.Map(assets, func(a Asset, index int) Asset {
		a.License = n.baseUrl + a.License
		a.LinuxAmd64.Url = n.baseUrl + a.LinuxAmd64.Url
		a.LinuxArm64.Url = n.baseUrl + a.LinuxArm64.Url
		a.WindowsAmd64.Url = n.baseUrl + a.WindowsAmd64.Url
		return a
	})
}

type NvidiaCudnn struct {
	baseUrl string
	Product string `json:"release_product"`
	Cudnn   Asset  `json:"Cudnn"`
}

func (n *NvidiaCudnn) Assets() []Asset {
	assets := []Asset{n.Cudnn}
	return lo.Map(assets, func(a Asset, index int) Asset {
		a.License = n.baseUrl + a.License
		a.LinuxAmd64.Url = n.baseUrl + a.LinuxAmd64.Url
		a.LinuxArm64.Url = n.baseUrl + a.LinuxArm64.Url
		a.WindowsAmd64.Url = n.baseUrl + a.WindowsAmd64.Url
		return a
	})
}

type Asset struct {
	Name         string        `json:"name"`
	License      string        `json:"license_path"`
	LinuxAmd64   VersionedFile `json:"linux-x86_64"`
	LinuxArm64   VersionedFile `json:"linux-aarch64"`
	WindowsAmd64 VersionedFile `json:"windows-x86_64"`
}

type File struct {
	Url string `json:"relative_path"`
}

type VersionedFile struct {
	File
}

// UnmarshalJSON If the JSON is `{ "cuda12": {...} }`, pick the cuda12 entry (or the only entry).
func (vf *VersionedFile) UnmarshalJSON(b []byte) error {
	// Case 1: direct File object
	var file File
	if err := json.Unmarshal(b, &file); err == nil && file.Url != "" {
		vf.File = file
		return nil
	}

	// Case 2: version map (e.g. {"cuda12": File, ...})
	var m map[string]File
	if err := json.Unmarshal(b, &m); err != nil {
		return err
	}

	// Look for the "cuda12" entry
	if v, ok := m["cuda12"]; ok {
		vf.File = v
		return nil
	}

	return nil
}
