## Why

`add-gui-enhance-1-seam` gave the application one run slot holding one result, and bounded its cost
deliberately: a new run displaces the previous one, drops its pixels, and that is what keeps resident
memory to a single result. `slot.rs` says so in as many words, and design.md D4 is the decision.

What neither covers is the case where **no next run ever comes**. `Run.result` is an
`opai::Picture` - an `Arc<DynamicImage>`, the whole decoded buffer - and the only thing that drops it
is `Runs::start` displacing it. Closing the file does not. Clearing the drawer does not. Switching to
another photograph does not. The frontend's `closeFile`/`closeAll` tell the per-file stores on their
own side and nothing at all on the Rust side, so the buffer stays resident until another run begins
or the process exits.

The number is not marginal. An 8x upscale of a 24 MP source is about 1.5 gigapixels; at RGBA that is
roughly **4.5 GB held for nothing**, on a machine that has just been asked to run inference. A user
who enhances a large scan, looks at it, closes it and carries on working is holding that until they
enhance something else.

`Runs.produced` has the same shape of problem at a much smaller scale: one `String` per enhanced
identity, never removed, for the life of the process.

This change gives the slot a release path and calls it from the one place that knows a result is no
longer wanted.

## What Changes

- **A held result can be released by name.** `Runs` gains a release for one identity and a release
  for everything, alongside the displacement it already does.
- **Closing an image releases what was enhanced from it.** Closing every image releases everything.
- **`produced` stops growing without bound.** It exists only to tell "this application made that
  identity and no longer holds it" from "that identity was never made", so it is bounded to a recent
  window rather than kept forever.
- **A released identity is reported as displaced, not unknown.** This is the existing distinction and
  the existing refusal: a stale `opai://` address the window is still drawing reads as "this is gone"
  rather than "you asked for something that never was". Nothing about the served response changes.

## Crates touched

`gui` only - both halves. The slot, the command and the frontend call are all GUI state: `opai` holds
no run slot and the CLI has no window to close an image in. Nothing here is logic a second front end
would want, so none of it belongs in `opai`.

## rust-sak

Checked, and there is nothing to use. This is lifetime management of a value the GUI already holds;
it involves no download, hashing, extraction, filesystem work, caching or hardware probe. `memo` is
the nearest module and is the wrong shape - it is a cache keyed by input, whereas this is a single
slot whose contents are dropped on an external event.

## Parity

The Wails application has no equivalent, because it does not hold the enhanced result in the backend
at all - it hands the pixels to the frontend, where the browser's own lifetime rules apply. Holding
them in Rust is this project's own decision (design.md D4, and `ipc/images.ts` records why a cache on
the frontend side was refused), so the release path is a cost that decision carries rather than a
parity gap. No user-visible behaviour changes.

## Task estimate

9 tasks. Well inside the 15-task budget; this is not a slice of a series.
