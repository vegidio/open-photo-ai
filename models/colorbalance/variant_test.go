package colorbalance

import "testing"

// TestMixedSpecChannelLayout pins the contract between the conversion script and ProcessMixed. These numbers are the
// graph's output layout: get them wrong and the weight planes are read out of the wrong offsets, which renders
// nonsense rather than failing. They are literals here for the same reason the operation ids are literals in
// models/ids_test.go - a derived expectation would move along with the bug.
func TestMixedSpecChannelLayout(t *testing.T) {
	spec := &MixedSpec{Settings: 3}

	if got := spec.Renderings(); got != 2 {
		t.Errorf("Renderings() = %d, want 2 (shade and tungsten; daylight is the input image)", got)
	}
	if got := spec.Channels(); got != 9 {
		t.Errorf("Channels() = %d, want 9 (3 weight planes + 2 renderings x 3)", got)
	}
	if got := spec.RenderingOffset(0); got != 3 {
		t.Errorf("RenderingOffset(0) = %d, want 3 (shade starts after the weight planes)", got)
	}
	if got := spec.RenderingOffset(1); got != 6 {
		t.Errorf("RenderingOffset(1) = %d, want 6 (tungsten)", got)
	}

	// Every rendering must sit inside the output, and the last one must end exactly at its edge.
	last := spec.RenderingOffset(spec.Renderings()-1) + 3
	if last != spec.Channels() {
		t.Errorf("the last rendering ends at channel %d but the output has %d", last, spec.Channels())
	}
}

// TestMixedSpecScalesWithSettings covers the `_D_S_T_F_C` case upstream also publishes. Nothing ships it today, but
// the layout is derived rather than hard-coded precisely so that adding it is a data change, and a formula that only
// works for one value of Settings would not be one.
func TestMixedSpecScalesWithSettings(t *testing.T) {
	spec := &MixedSpec{Settings: 5}

	if got := spec.Renderings(); got != 4 {
		t.Errorf("Renderings() = %d, want 4", got)
	}
	if got := spec.Channels(); got != 17 {
		t.Errorf("Channels() = %d, want 17 (5 weight planes + 4 renderings x 3)", got)
	}
	for i := range spec.Renderings() {
		if got, want := spec.RenderingOffset(i), 5+3*i; got != want {
			t.Errorf("RenderingOffset(%d) = %d, want %d", i, got, want)
		}
	}
}
