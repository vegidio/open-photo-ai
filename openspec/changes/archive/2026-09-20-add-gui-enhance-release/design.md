## Slices

Not a slice of a series. It is a single change closing a lifetime gap left open by
`add-gui-enhance-1-seam`, which is archived.

For context, the enhance series as it stands:

| Slice | Delivers | Depends on | State |
| --- | --- | --- | --- |
| `add-gui-enhance-1-seam` | The wire shape, the run slot, the run/stop commands, progress, `opai://` for results | - | Done |
| `add-gui-enhance-2-sidebar` | The screen: the per-file stack, the add menu, upscale's options, the chip, the enhanced pane | Slice 1 | Done |
| `add-gui-enhance-release` | Releasing a held result when the image it belongs to closes | Slices 1 and 2 | **This change** |

## How the work divides

`gui` only, on both sides of the seam:

- `crates/gui/src/enhance/slot.rs` - the release itself, and bounding `produced`.
- `crates/gui/src/enhance/mod.rs` - the two commands, beside `cancel_enhance`.
- `crates/gui/src/enhance/run.rs` - one line: the run tells the slot which image it is over (D1).
- `crates/gui/frontend/ipc/enhance.ts` - the wrapper, and the command name's second spelling.
- `crates/gui/frontend/stores/enhancements.ts` - where the call is made from.

Nothing goes in `opai`. The run slot is GUI state: `opai` is handed a picture and hands one back, and
knows nothing about which of them a window is still looking at.

## D1 - The release is named, like the stop

`cancel_enhance` names its run rather than stopping "whatever is running", because `invoke` calls race
(design.md D3 of slice 1). The release has the same problem and takes the same answer: it names the
**identity** whose pixels are no longer wanted, and does nothing unless the slot is holding that one.

Naming it is what makes the race safe. Closing image A while a run over image B is in flight is
ordinary, and a release that meant "drop whatever you hold" would throw away B's result. A named
release cannot: the slot compares before it drops.

The frontend already has the identity in hand - it is on the `ImageRecord` the file store is removing -
so nothing has to be looked up to make the call.

**The identity named is the source photograph's, not the result's.** Those are different strings: a result's
identity is composed from the source's plus every operation applied (`identity_after` in `opai`), and the
frontend holds no per-file record of it - the enhanced address lives in the preview pane's own state and goes
when the pane does. So the slot is what has to remember the pairing: `Run` carries the identity of the image
it is over, written by `start`, and the release compares against that.

The released result's own identity is the one that stays in `produced`, which is what keeps it answering as
displaced rather than unknown (D2). Nothing about `resolve` changes.

## D2 - Released is displaced, not unknown

`Resident` already distinguishes a result this application produced and no longer holds (`Displaced`,
answered as `410 Gone`) from an identity it never made (`Refusal::Unknown`, `404`). A release produces
exactly the first state, and reuses it unchanged.

This is why `produced` cannot simply be emptied: it is the record that makes the distinction possible.
A window still drawing a released address must be told the result is gone rather than that it never
existed, which is the difference between a stale pane and a bug report.

## D3 - `produced` is bounded, not unbounded

`produced` is a `HashSet<String>` that only ever grows - one 16-character identity per enhanced result,
for the process's life. It is small, but it is unbounded, and its only job is answering "did this
application ever make that?" for an address a **window** is still holding.

A window cannot hold an address indefinitely: it is holding one because a pane is drawing it, and a
pane is drawing it because the run that produced it was recent. So a bounded ring of recent identities
answers every question that is actually asked, and an identity that falls out of the ring answers
`Unknown` rather than `Displaced` - a slightly worse message for an address nothing can still be
drawing.

Size is a judgement, not a measurement: one entry per enhanced result, and the interface holds one
result. A small ring (64) is three orders of magnitude more than the seam can produce between two
releases, and still bounded.

## D4 - Where the call is made from

The frontend already has the seam for this. `stores/files.ts` carries a `FileOwner` registry: a store
holding per-file state registers a `forget(path, identity)` and a `forgetAll()`, and closing a file
iterates the owners. The enhancement store is already one of them.

So the release is not a new call site threaded into `closeFile` - it is the existing owner's `forget`
gaining a line. That matters because the registry is what guarantees every per-file holder is told:
a release wired directly into `closeFile` would be the fifth thing that has to be remembered there,
which is the arrangement the registry replaced.

The call is fire-and-forget. A release that fails has not lost anything the user can see - the pixels
stay resident until the next run displaces them, which is exactly today's behaviour - so it is logged
and not surfaced.

## Ownership and lifetime

`Run.result` is an `Option<opai::Picture>`; `Picture` holds its pixels behind an `Arc`. Dropping the
`Option` drops this slot's handle, and the buffer is freed when the last handle goes.

The other handles are short-lived and already accounted for: `serve.rs` clones the `Picture` out of the
slot under the lock and decodes from it outside, so a release arriving mid-request leaves that request
holding the last handle and freeing it when it answers. The request still succeeds, which is correct -
it was entitled when it was claimed.

No ONNX session, model pool or execution provider is touched. A run in flight owns its own session
through `opai`; releasing a **finished** run's pixels cannot reach one. Releasing while a run is in
flight does not stop it, and `start` already cancels rather than drops.

## D5 - A released run in flight is marked, not merely skipped

Releasing while the named run is still working has nothing to drop, and the obvious reading is that it
should therefore do nothing at all: the window sends `cancel_enhance` when the pane goes, and a
cancelled run produces nothing to hold.

That reading is wrong, and `run.rs` already says why in its own comment: **cancellation is cooperative
and checked between tiles**, so a run can answer `Ok` without ever noticing it was cancelled - a
fully-cached chain does no tiling to notice between, and the cache is on by default, so a user
re-running a chain they have run before is exactly the case that reaches it. `stop` leaves the run in
the slot, so `finish` would then match on its id and write the pixels of a photograph that is no longer
open into the slot, a moment after the release that existed to prevent that, with nothing left to
release them afterwards. The leak this change closes would reopen in the narrow case, at full size.

So the release **marks** the run it names (`Run.released`) rather than merely finding nothing to drop,
and `finish` drops what a marked run hands back and reports it as stopped - the same answer, and the
same reasoning, as a run that finished after being displaced. The mark lands only on a run whose source
the release names, so closing one photograph still cannot throw away the result of a run over another.

## Platforms

The lifetime behaviour is platform-independent - it is a `Drop`. Validated on macOS (arm64), which is
the development target. Windows and Linux are untested for this change specifically and carry no
platform-specific code.
