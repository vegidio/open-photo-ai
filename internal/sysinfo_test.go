package internal

import (
	"testing"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/sysinfo"
)

// stubMemoryInfo and stubGPUInfo swap the memoized probes for the duration of a test. The probes are plain vars rather
// than functions precisely so this is possible without a machine of each size.
func stubMemoryInfo(t *testing.T, info sysinfo.MemoryInfo, err error) {
	t.Helper()

	original := MemoryInfo
	MemoryInfo = func() (sysinfo.MemoryInfo, error) { return info, err }

	t.Cleanup(func() { MemoryInfo = original })
}

func stubGPUInfo(t *testing.T, gpus []sysinfo.GPUInfo, err error) {
	t.Helper()

	original := GPUInfo
	GPUInfo = func() ([]sysinfo.GPUInfo, error) { return gpus, err }

	t.Cleanup(func() { GPUInfo = original })
}

// TestTotalRAMBytes pins the unit conversion, which is the part with no visible symptom when it is wrong.
//
// sysinfo.MemoryInfo documents its field as bytes but every backend divides by 1,000,000 before returning it, so the
// value is decimal megabytes. Reading it as bytes makes a 64 GB machine look like 68 KB of RAM and silently collapses
// the host budget to its floor - models get rebuilt more than they should be and nothing else says so. The figures
// below are what sysctl/proc/CIM report for machines of those sizes.
func TestTotalRAMBytes(t *testing.T) {
	const gib = int64(1) << 30

	tests := []struct {
		name       string
		reportedMB uint64
		want       int64
	}{
		{"16 GB machine", 17179, 16 * gib},
		{"32 GB machine", 34359, 32 * gib},
		{"64 GB machine", 68719, 64 * gib},
	}

	// The reported figures are truncated decimal megabytes, so they land a hair under the round GiB values rather than
	// exactly on them. A tolerance keeps the test about the unit - the failure mode is off by a factor of a million,
	// not by a few MiB.
	const tolerance = 64 * (1 << 20)

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			stubMemoryInfo(t, sysinfo.MemoryInfo{Total: tt.reportedMB}, nil)

			got := TotalRAMBytes()
			if diff := got - tt.want; diff > tolerance || diff < -tolerance {
				t.Errorf("TotalRAMBytes() = %.3f GiB, want %.3f GiB",
					float64(got)/float64(gib), float64(tt.want)/float64(gib))
			}
		})
	}

	t.Run("a failed probe reports zero", func(t *testing.T) {
		stubMemoryInfo(t, sysinfo.MemoryInfo{Total: 17179}, errors.New("no"))

		if got := TotalRAMBytes(); got != 0 {
			t.Errorf("TotalRAMBytes() = %d, want 0 when the probe failed", got)
		}
	})
}

// TestLargestVRAMBytes covers the other unit: GPU memory is reported in MiB, not in the decimal megabytes system RAM
// uses. It also pins "largest, not sum" - a model is built on one card, so the budget describes the card it lands on.
func TestLargestVRAMBytes(t *testing.T) {
	const mib = int64(1) << 20

	tests := []struct {
		name   string
		gpus   []sysinfo.GPUInfo
		err    error
		want   int64
		wantOk bool
	}{
		{
			name:   "a single card converts from MiB",
			gpus:   []sysinfo.GPUInfo{{Name: "RTX 4080", Memory: 16384}},
			want:   16384 * mib,
			wantOk: true,
		},
		{
			name:   "two cards report the largest, not the sum",
			gpus:   []sysinfo.GPUInfo{{Name: "RTX 4080", Memory: 16384}, {Name: "RTX 3060", Memory: 12288}},
			want:   16384 * mib,
			wantOk: true,
		},
		{
			// go-sak deliberately reports 0 on the Windows CIM path, so this is the real AMD/Intel DirectML case.
			name:   "a card that reports nothing is not ok",
			gpus:   []sysinfo.GPUInfo{{Name: "Radeon", Memory: 0}},
			want:   0,
			wantOk: false,
		},
		{
			name:   "an integrated part alongside a discrete one still finds the discrete one",
			gpus:   []sysinfo.GPUInfo{{Name: "UHD Graphics", Memory: 0}, {Name: "RTX 3060", Memory: 12288}},
			want:   12288 * mib,
			wantOk: true,
		},
		{
			name:   "a failed probe is not ok",
			err:    errors.New("no"),
			want:   0,
			wantOk: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			stubGPUInfo(t, tt.gpus, tt.err)

			got, ok := LargestVRAMBytes()
			if got != tt.want || ok != tt.wantOk {
				t.Errorf("LargestVRAMBytes() = (%d, %t), want (%d, %t)", got, ok, tt.want, tt.wantOk)
			}
		})
	}
}
