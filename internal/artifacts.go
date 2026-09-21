package internal

import "github.com/vegidio/go-sak/sysinfo"

// Releases pins each dependency to one published release: the tag it comes from, and the SHA-256 and size of that
// release's archive on every platform it is built for.
//
// The hash is of the `.7z` as published, not of anything inside it, so a single value covers every file the archive
// carries - which is the only way a runtime shipped alongside its execution providers, or a CUDA tree of several
// hundred files, can be verified at all. Pinning it here rather than fetching a checksum at install time is what makes
// the check worth having: a compromised release cannot move the goalposts.
//
// The tag is the version this build installs, and this is the only place it is written down. Nothing else names a
// release - the shared library's file name, which carries the version too, is the Lib field beside its hash rather
// than a per-platform constant elsewhere - so a bump is an edit here and nowhere else. Grouping by dependency rather than by archive name is what
// makes that safe: the tag is written once per dependency instead of once per platform, and two versions of the same
// dependency cannot be expressed at all.
//
// To bump one, change its Tag and replace its hashes together. The values are printed by:
//
//	gh api repos/vegidio/open-photo-ai/releases/tags/cuda%2F13.3.0 \
//	  --jq '.assets[] | "\(.name) \(.size) \(.digest)"'
//
// The digest GitHub reports there is computed on upload and is the same value a local sha256sum of the archive gives.
var Releases = map[string]Release{
	"onnx": {
		Tag: "runtime/1.26.0",
		Archives: map[string]Artifact{
			"darwin_arm64":  {Hash: "5cafbaaef5eb43142499fc4ef1087024b6b33a6bd17d4183da6f4cb6079fcb47", Size: 6417404, Lib: "onnxruntime.1.26.0.dylib"},
			"linux_amd64":   {Hash: "e2062cdc87ab593bcd541d2b5fbad6771e726aed6a4375352b98578ecb747236", Size: 174834737, Lib: "onnxruntime.so.1.26.0"},
			"linux_arm64":   {Hash: "1e7e96f673874f7216b4736a65b8967e129189f24da11c797432d1cd2e33761a", Size: 4879338, Lib: "onnxruntime.so.1.26.0"},
			"windows_amd64": {Hash: "2aa448485be581d211a53023520785af13443a31c3f82e44b9f01a2ff457b22d", Size: 172592865, Lib: "onnxruntime-1.26.0.dll"},
			"windows_arm64": {Hash: "d4a62a7dcfe10872f67135be34e49bf1714d041e3cd81816666a503f53dbd327", Size: 3444425, Lib: "onnxruntime-1.26.0.dll"},
		},
	},

	// The WebGPU execution provider is a plugin library rather than part of the runtime archive above, and it comes
	// from Microsoft directly: these are the official `onnxruntime-ep-webgpu` wheels from PyPI, repackaged unmodified
	// into this table's archive layout by scripts/package_webgpu.py, which also prints the values below.
	//
	// Nothing else is needed on the machine beyond the platform's GPU driver: Dawn, the WebGPU implementation, is
	// linked in, and it talks Vulkan on Linux, Direct3D 12 on Windows and Metal on macOS.
	"webgpu": {
		// The plugin is compiled against one release of the runtime's plugin API and the runtime refuses kernels from a
		// plugin newer than itself, so this tag moves with the "onnx" tag above and never ahead of it: 0.1.0 targets
		// 1.26, 0.2.1 the 1.27/1.28 API, 0.3.0 the 1.29 API. The mismatch is not caught at registration - the library
		// loads and publishes its device - but at the first session build, which fails every model over to the CPU.
		Tag: "webgpu/0.1.0",
		Archives: map[string]Artifact{
			"darwin_arm64":  {Hash: "f94da69cb9ecab1657ca666856ef26f33ef88fd687412f9d67d7391bb72dfb2c", Size: 2255113, Lib: "libonnxruntime_providers_webgpu.dylib"},
			"linux_amd64":   {Hash: "ce5f61111e3ab7db769f262c6a4496f6c5ce3a1e99e516388edbfa724cfe81c6", Size: 2663210, Lib: "libonnxruntime_providers_webgpu.so"},
			"windows_amd64": {Hash: "ea501e433785695252b4bc8bedc9f21fbf37e031b671a9101f738f9354abd517", Size: 2296612, Lib: "onnxruntime_providers_webgpu.dll"},
		},
	},

	"cuda": {
		Tag: "cuda/13.3.0",

		// Turing. CUDA 13.0 removed offline compilation for sm_50 through sm_72, so this floor moves with the tag
		// above: a bump back to a 12.x release would have to lower it to 5.0, and a later major may raise it again.
		MinComputeCapability: sysinfo.ComputeCapability{Major: 7, Minor: 5},

		Archives: map[string]Artifact{
			"linux_amd64":   {Hash: "fb4eb6ef362973c6cdefeae43e09cea05270288acbdd47cc0fc4f50ca6bc47c0", Size: 587322552},
			"linux_arm64":   {Hash: "ed5eb63005e30a098c270b2020b66eab87a5142d9936886fe820c19693487a24", Size: 697083065},
			"windows_amd64": {Hash: "b85d858e4fd97bab2c808c7ad7106a89dc8dfba535a0f20f63c4decc4d24452b", Size: 585185362},
		},
	},

	"cudnn": {
		Tag: "cudnn/9.23.1",
		Archives: map[string]Artifact{
			"linux_amd64":   {Hash: "18e84817dd836046087ece4b6776fea066440854c48b6b6e6c4a388b43df4174", Size: 407298251},
			"linux_arm64":   {Hash: "92537e1ed61579840b12282c70633f0ef6bfd3929d38943e31ebcd6b699b62d6", Size: 507936847},
			"windows_amd64": {Hash: "a26f54c17ea990e59a6f7232bdebf39f58b5debd9a0c8ec9a6573885ae27edbd", Size: 354760387},
		},
	},

	"tensorrt": {
		Tag: "tensorrt/10.14.1",

		// Turing, and for a second reason than CUDA's: TensorRT 10.5 removed Volta, so 7.0 is out on this release's
		// own terms as well as the toolkit's. It can never be lower than the CUDA floor above, since the TensorRT
		// execution provider is built on the CUDA one.
		MinComputeCapability: sysinfo.ComputeCapability{Major: 7, Minor: 5},

		Archives: map[string]Artifact{
			"linux_amd64":   {Hash: "d2a29e4fbc78445ae715ff7ff6fde6f59ba6425a8b0017753f3a776096c59376", Size: 1780420117},
			"linux_arm64":   {Hash: "28a7699c9208d6d1a5f77f65bd2dad9547a6245599ae5192c8a1baf4ef1496a0", Size: 1993059105},
			"windows_amd64": {Hash: "1156bf236dd66aa7c4772f8144599b81061512d035d05f02c037e1a1ad20370b", Size: 1399566073},
		},
	},
}
