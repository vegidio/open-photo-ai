## Why

`SourceImage` is the crate's one description of an image the application is working on. Its own
documentation says so:

> This is the one description of "an image the application is working on" that inference, the
> per-operation cache and the export queue all work from.

Its name says the opposite. `Source` reads as "the thing you start with", and so far nothing has
contradicted that: the type has exactly one producer, `image::load`, and travels in one direction.

The next change ends that. `add-inference-5-kyoto` returns this type — an enhanced image is the same
three things, and has to be, because a chain runs an operation over the result of the one before it
and the per-operation cache reads and writes one shape. A type named `SourceImage` appearing as the
**return** of `Opai::process` tells a reader the wrong thing at exactly the moment the public surface
gains its first consumer.

**The name was never argued.** The `add-image-io` design justified the *field* it prefixes:

> ### The destination is a parameter, not a field
> `save` takes the destination path from the caller. `SourceImage.path` means only where the pixels
> came from.

That decision is right and is untouched here — it fixes a real defect in the Go implementation, where
`SaveImage` wrote to `ImageData.FilePath` and every call site built a throwaway value carrying an
output path and an empty hash. What is being corrected is that the fix to one field's semantics was
spelled into the name of the whole type, which has a wider role than that field.

Now is the cheapest this will ever be: 25 references across 6 files, one producer, and no front end or
cache built against it yet.

## What Changes

**In `opai`, a rename and a documentation correction. No behaviour changes at all.**

- **`SourceImage` becomes `Picture`**, with `image/source.rs` becoming `image/picture.rs`. `Picture` is
  already the word the specs use for the thing itself — `inference-tiling` and `image-enhancement`
  both say "no picture is produced" and "combined into one picture" — it never reads as input-only,
  and it collides with nothing: it sits beside the `opai::image` module and `DynamicImage` without
  ambiguity.
- **The identity documentation is corrected to match what the capability actually requires.** The
  type's doc comment currently states, as a property of the type, that the identity is *"the XXH3-64
  of the whole of the file's bytes"*. That is the **loader's** guarantee, not the type's: the next
  change produces images whose identity is derived from an input identity plus the operations applied,
  and no file on disk hashes to it. The doc is reworded to say what identifies the pixels and where it
  comes from in each case, keeping the invariant that actually matters — that the identity describes
  *these* pixels and is never carried onto different ones.

Every field, accessor, invariant and guarantee is unchanged. `path` still means where the pixels came
from and never where they are going. The fields stay private, `dimensions()` stays derived, and
`Picture::new` remains the single reviewed place the identity invariant can be broken.

**BREAKING** for any consumer naming the type — which today is one integration test inside this
repository. Nothing outside it exists yet.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None, and `skip_specs: true` is set on that basis rather than as a convenience.

`image-io`'s requirements are already correctly scoped and none of them moves. The identity
requirement opens *"A loaded image SHALL carry an XXH3-64 identity computed over the whole of the
file's bytes"* — every clause of it is about loading, which is still exactly true. Its closing clause,
*"The identity SHALL describe the bytes that were read, and SHALL NOT be silently carried onto
different pixels"*, is the rule the next change's derived identity **honours** rather than breaks:
composing a new identity is what stops the input's being carried onto computed pixels.

The main specs name no Rust type anywhere, being worded against observable behaviour, so nothing in
`openspec/specs/` mentions `SourceImage` at all. That is the check that this is a rename and not a
behaviour change.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` name neither the old type nor the new one — neither
front end has been wired to image IO yet.

**`rust-sak`:** untouched. No infrastructure is added, removed or reached for; this change writes no
new logic of any kind.

**New third-party crates:** none.

**Parity divergence from the Go implementation:** none introduced. The Go type is `types.ImageData`
and this one has never shared its name; the divergence that matters — the destination being a
parameter rather than a field — is the `add-image-io` decision this change deliberately leaves alone.

**Scope of the edit:** 25 references across 6 files — `image/source.rs` (renamed to `image/picture.rs`),
`image/mod.rs`, `image/load.rs`, `image/save.rs`, `lib.rs` and `tests/public_api.rs`. Zero references
in `openspec/specs/`.

**Why its own change rather than part of the inference slice:** it edits code belonging to an archived
capability, which `add-inference-5-kyoto` states it does not touch, and it follows the three
`refactor-*` changes already in the archive at the same granularity. Doing it first also keeps that
slice at 14 tasks rather than pushing it to the 15-task limit for work that is not its own.

**Not in scope:** any change to what `load`, `save` or `probe` do; the destination-as-a-parameter
decision; the identity's algorithm or its format; and the derived identity itself, which is
`add-inference-5-kyoto`'s to introduce.
