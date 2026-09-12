package utils

import (
	"testing"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/sysinfo"
	"github.com/vegidio/open-photo-ai/internal"
)

// stubGPUInfo swaps the memoized probe for the duration of a test. internal.GPUInfo is a var precisely so this is
// possible without a machine of each generation.
func stubGPUInfo(t *testing.T, gpus []sysinfo.GPUInfo, err error) {
	t.Helper()

	original := internal.GPUInfo
	internal.GPUInfo = func() ([]sysinfo.GPUInfo, error) { return gpus, err }

	t.Cleanup(func() { internal.GPUInfo = original })
}

func nvidia(name string, major, minor int) sysinfo.GPUInfo {
	return sysinfo.GPUInfo{
		Name:              name,
		Vendor:            "NVIDIA",
		ComputeCapability: sysinfo.ComputeCapability{Major: major, Minor: minor},
	}
}

// TestIsCudaSupported pins the architecture gate against the cards that actually reported the failure in production.
//
// Every GPU here was seen in the field. The four Pascal cards are the ones whose sessions died with CUBLAS failure 8
// ("the function requires an architectural feature absent from the device") after downloading the whole CUDA tree,
// and the GTX 16-series entries are why this cannot be a model-name heuristic: a GTX 1660 is Turing and works, a GTX
// 1060 is Pascal and cannot, and the names differ by one digit.
func TestIsCudaSupported(t *testing.T) {
	tests := []struct {
		name string
		gpu  sysinfo.GPUInfo
		want bool
	}{
		// Below the CUDA 13 floor: dropped when the toolkit removed sm_50 through sm_72.
		{"GTX 1060 6GB is Pascal", nvidia("NVIDIA GeForce GTX 1060 6GB", 6, 1), false},
		{"GTX 1080 Ti is Pascal", nvidia("NVIDIA GeForce GTX 1080 Ti", 6, 1), false},
		{"MX250 is Pascal", nvidia("NVIDIA GeForce MX250", 6, 1), false},
		{"MX130 is Maxwell", nvidia("NVIDIA GeForce MX130", 5, 0), false},
		{"GTX 750 Ti is Maxwell", nvidia("NVIDIA GeForce GTX 750 Ti", 5, 0), false},
		{"GTX 650 is Kepler", nvidia("NVIDIA GeForce GTX 650", 3, 0), false},
		{"Titan V is Volta, the last generation dropped", nvidia("NVIDIA TITAN V", 7, 0), false},

		// At or above the floor.
		{"GTX 1650 is Turing despite the GTX name", nvidia("NVIDIA GeForce GTX 1650", 7, 5), true},
		{"GTX 1660 SUPER is Turing despite the GTX name", nvidia("NVIDIA GeForce GTX 1660 SUPER", 7, 5), true},
		{"RTX 2060 is Turing", nvidia("NVIDIA GeForce RTX 2060", 7, 5), true},
		{"RTX 3090 is Ampere", nvidia("NVIDIA GeForce RTX 3090", 8, 6), true},
		{"RTX 5090 is Blackwell", nvidia("NVIDIA GeForce RTX 5090", 12, 0), true},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			stubGPUInfo(t, []sysinfo.GPUInfo{test.gpu}, nil)

			if got := IsCudaSupported(); got != test.want {
				t.Errorf("IsCudaSupported() = %v, want %v for %s (compute %s)",
					got, test.want, test.gpu.Name, test.gpu.ComputeCapability)
			}
		})
	}
}

// A card whose capability could not be read keeps CUDA, which is the pre-gate behaviour. See canTarget for why the
// unknown case fails open rather than closed.
func TestIsCudaSupportedAllowsUnknownCapability(t *testing.T) {
	stubGPUInfo(t, []sysinfo.GPUInfo{{Name: "NVIDIA GeForce RTX 3090", Vendor: "NVIDIA"}}, nil)

	if !IsCudaSupported() {
		t.Error("IsCudaSupported() = false for a GPU with no capability reported, want true")
	}
}

// The vendor field is empty on the Windows CIM fallback often enough that the name has to carry the check.
func TestIsCudaSupportedRecognizesNvidiaByName(t *testing.T) {
	stubGPUInfo(t, []sysinfo.GPUInfo{{Name: "NVIDIA GeForce RTX 4070 Laptop GPU"}}, nil)

	if !IsCudaSupported() {
		t.Error("IsCudaSupported() = false for an NVIDIA GPU named only in the product string, want true")
	}
}

// A machine with an old NVIDIA card alongside a new one can still use CUDA: the session is built on one device, and
// the new card is a device the toolkit can target.
func TestIsCudaSupportedAcceptsOneUsableCard(t *testing.T) {
	stubGPUInfo(t, []sysinfo.GPUInfo{
		nvidia("NVIDIA GeForce GTX 1080 Ti", 6, 1),
		nvidia("NVIDIA GeForce RTX 4090", 8, 9),
	}, nil)

	if !IsCudaSupported() {
		t.Error("IsCudaSupported() = false with a supported card present, want true")
	}
}

func TestIsCudaSupportedWithoutNvidia(t *testing.T) {
	stubGPUInfo(t, []sysinfo.GPUInfo{{Name: "AMD Radeon RX 7900 XTX", Vendor: "AMD"}}, nil)

	if IsCudaSupported() {
		t.Error("IsCudaSupported() = true with no NVIDIA GPU, want false")
	}
}

// A failed probe is not an NVIDIA machine as far as this is concerned; internal.GPUInfo has already logged why.
func TestIsCudaSupportedWhenProbeFails(t *testing.T) {
	stubGPUInfo(t, nil, errors.New("no GPU probe available"))

	if IsCudaSupported() {
		t.Error("IsCudaSupported() = true after a failed GPU probe, want false")
	}
}

// The floor is read from the pinned artifact table rather than written into the gate, so that a CUDA bump moves both
// together. This pins the two to each other: if the tag moves, this test is the thing that notices.
func TestCudaFloorMatchesPinnedRelease(t *testing.T) {
	floor, found := internal.MinComputeCapability("cuda")
	if !found {
		t.Fatal("the pinned CUDA release declares no compute capability floor")
	}

	tag, _ := internal.ReleaseTag("cuda")
	if floor.Major != 7 || floor.Minor != 5 {
		t.Errorf("CUDA floor is %s for %s; CUDA 13.x supports sm_75 and newer, so confirm the floor still matches the tag",
			floor, tag)
	}
}

// Dependencies with no architecture floor must read as unconstrained, not as "nothing is supported". onnx runs on the
// CPU everywhere and cuDNN is pulled in alongside CUDA rather than chosen, so neither declares one.
func TestNoFloorForDependenciesWithoutOne(t *testing.T) {
	for _, prefix := range []string{"onnx", "cudnn", "nonexistent"} {
		if _, found := internal.MinComputeCapability(prefix); found {
			t.Errorf("MinComputeCapability(%q) reported a floor; only CUDA and TensorRT declare one", prefix)
		}
	}
}

// TestIsTensorRtSupported pins the cases the old model-name heuristic got wrong in both directions.
//
// It matched "rtx 20" through "rtx 50" against the product name, so every Turing card NVIDIA did not brand RTX was
// refused - the GTX 1650 and 1660 are compute capability 7.5 and squarely supported - while the professional lines
// were refused for having no "rtx <decade>" in their names at all. The 14 GTX 16-series sessions in the production
// logs are the concrete cost of the first half.
func TestIsTensorRtSupported(t *testing.T) {
	tests := []struct {
		name string
		gpu  sysinfo.GPUInfo
		want bool
	}{
		// Turing without the RTX brand: the old heuristic said no, TensorRT says yes.
		{"GTX 1650 is Turing", nvidia("NVIDIA GeForce GTX 1650", 7, 5), true},
		{"GTX 1650 Ti is Turing", nvidia("NVIDIA GeForce GTX 1650 Ti with Max-Q Design", 7, 5), true},
		{"GTX 1660 SUPER is Turing", nvidia("NVIDIA GeForce GTX 1660 SUPER", 7, 5), true},
		{"Titan RTX is Turing", nvidia("NVIDIA TITAN RTX", 7, 5), true},

		// Professional cards, named without an "rtx <decade>" the old check could match.
		{"Quadro RTX 5000 is Turing", nvidia("Quadro RTX 5000", 7, 5), true},
		{"RTX A4000 is Ampere", nvidia("NVIDIA RTX A4000", 8, 6), true},
		{"RTX 6000 Ada is Ada", nvidia("NVIDIA RTX 6000 Ada Generation", 8, 9), true},

		// Consumer cards the old check already accepted, which must keep working.
		{"RTX 2060 is Turing", nvidia("NVIDIA GeForce RTX 2060", 7, 5), true},
		{"RTX 3060 is Ampere", nvidia("NVIDIA GeForce RTX 3060", 8, 6), true},
		{"RTX 5090 is Blackwell", nvidia("NVIDIA GeForce RTX 5090", 12, 0), true},

		// Below the floor.
		{"Titan V is Volta, removed in TensorRT 10.5", nvidia("NVIDIA TITAN V", 7, 0), false},
		{"GTX 1080 Ti is Pascal", nvidia("NVIDIA GeForce GTX 1080 Ti", 6, 1), false},
		{"GTX 1060 6GB is Pascal", nvidia("NVIDIA GeForce GTX 1060 6GB", 6, 1), false},
		{"MX250 is Pascal", nvidia("NVIDIA GeForce MX250", 6, 1), false},
		{"GTX 750 Ti is Maxwell", nvidia("NVIDIA GeForce GTX 750 Ti", 5, 0), false},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			stubGPUInfo(t, []sysinfo.GPUInfo{test.gpu}, nil)

			if got := IsTensorRtSupported(); got != test.want {
				t.Errorf("IsTensorRtSupported() = %v, want %v for %s (compute %s)",
					got, test.want, test.gpu.Name, test.gpu.ComputeCapability)
			}
		})
	}
}

func TestIsTensorRtSupportedWithoutNvidia(t *testing.T) {
	stubGPUInfo(t, []sysinfo.GPUInfo{{Name: "AMD Radeon RX 7900 XTX", Vendor: "AMD"}}, nil)

	if IsTensorRtSupported() {
		t.Error("IsTensorRtSupported() = true with no NVIDIA GPU, want false")
	}
}

// The TensorRT execution provider is built on the CUDA one, so a card CUDA cannot target cannot run TensorRT either.
// Both floors are pinned by hand next to their tags, and nothing at runtime enforces the relationship between them -
// this does, so a future bump that lowered TensorRT's floor without lowering CUDA's fails here instead of shipping.
func TestTensorRtFloorIsNotBelowCuda(t *testing.T) {
	cuda, found := internal.MinComputeCapability("cuda")
	if !found {
		t.Fatal("the pinned CUDA release declares no compute capability floor")
	}

	tensorrt, found := internal.MinComputeCapability("tensorrt")
	if !found {
		t.Fatal("the pinned TensorRT release declares no compute capability floor")
	}

	if !tensorrt.AtLeast(cuda.Major, cuda.Minor) {
		t.Errorf("TensorRT floor %s is below the CUDA floor %s; TensorRT runs on top of CUDA and cannot support a card CUDA does not",
			tensorrt, cuda)
	}
}

// TensorRT is the larger download of the two, so anything it is offered for must also be offered CUDA. This is a
// consequence of the floors rather than of the code, which is why it is checked over real cards.
func TestTensorRtImpliesCuda(t *testing.T) {
	cards := []sysinfo.GPUInfo{
		nvidia("NVIDIA GeForce GTX 1060 6GB", 6, 1),
		nvidia("NVIDIA GeForce GTX 1660 SUPER", 7, 5),
		nvidia("NVIDIA TITAN V", 7, 0),
		nvidia("NVIDIA GeForce RTX 4090", 8, 9),
		nvidia("NVIDIA GeForce RTX 5090", 12, 0),
	}

	for _, card := range cards {
		stubGPUInfo(t, []sysinfo.GPUInfo{card}, nil)

		if IsTensorRtSupported() && !IsCudaSupported() {
			t.Errorf("%s is offered TensorRT but not CUDA", card.Name)
		}
	}
}
