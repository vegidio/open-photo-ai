## Context

`SourceImage` lives in `crates/opai/src/image/source.rs`: a private `PathBuf`, an
`Arc<DynamicImage>` and a `String` identity, with accessors, a derived `dimensions()`, and a
constructor documented as the single reviewed place where pixels and an identity meet without having
been read from the same file. It is re-exported from `lib.rs` and named in 25 places across 6 files,
all inside this repository.

It has one producer today — `image::load` — which is why the name has not yet been wrong in practice.
`add-inference-5-kyoto` adds a second, and that one is a **return**.

See proposal.md - Why for motivation. There is no spec delta; proposal.md - Capabilities records why,
and that reasoning is the load-bearing part of this change rather than anything below.

## Goals / Non-Goals

**Goals:**

- The type's name matches the role its own documentation already assigns it, before a second producer
  and two front ends are built against the current one.
- The identity's documentation says what is true of every value of the type, not what is true only of
  the ones the loader makes.
- The change is provably behaviour-free: the existing suite passes unmodified except where it spells
  the name.

**Non-Goals:**

- Any change to the type's shape. No field is added, removed, re-typed or made public.
- Any change to `load`, `save` or `probe`.
- Introducing the derived identity. This change makes the documentation able to describe one;
  `add-inference-5-kyoto` is what produces one.
- Revisiting the destination-as-a-parameter decision, which is correct and is the reason the `Source`
  prefix existed at all.

## Decisions

### D1. `Picture`, and why not the three alternatives

`Picture` is already the word the specs use for the thing itself. `inference-tiling` says *"no picture
is produced"* and *"combined into one picture with no visible seam"*; `image-enhancement` says *"no
picture is produced"* in three separate scenarios. Adopting the vocabulary the specs already settled on
costs nothing and makes the code and the specs read as one document.

It also has the property the current name lacks and which is the whole point: it is directionless. A
picture is a picture whether it was decoded from a file or computed by a model, which is exactly the
relationship `load` and `process` have to it.

The alternatives were weighed:

- **`WorkingImage`** — the literal phrase from the type's own doc comment, and the most defensible as a
  reading of recorded intent. Rejected on how it reads at every call site: `&WorkingImage` as a
  parameter and `Result<WorkingImage, _>` as a return are noisier than what they describe, and the
  "working" is redundant once the type is the only image the crate passes around.
- **`ImageData`** — the Go app's own name, which would carry the domain vocabulary across. Rejected
  because `Data` says nothing (every type is data), and because the Go type's name is attached to the
  defect this design already fixed — reusing it invites a reader to assume the field semantics came
  across too.
- **`Image`** — shortest, but it collides in a reader's head three ways: with the `opai::image`
  module it would be re-exported beside, with the `image` crate, and with `image::DynamicImage` which
  is one of its own fields.

The file moves with the type: `image/source.rs` becomes `image/picture.rs`, since the module is named
for what it holds.

### D2. The identity documentation describes the value, not just the loader

The type currently documents its identity as a property of the type:

> `identity` is the XXH3-64 of the **whole** of the file's bytes… That equality is the point of it: a
> file listing can hash a path without decoding it, and later recognise the loaded image as the same
> one.

That is true of every `SourceImage` in existence today and will be false of the first one
`Opai::process` returns, which carries an identity composed from its input's plus the operations
applied. No file on disk hashes to that value, and nothing is wrong with it — it is what lets a
derived image be recognised, and what the per-operation cache will be keyed on.

The rewording separates the two so that neither is overstated:

- **What is invariant, for every value:** the identity describes *these* pixels. It is never carried
  onto different ones, and it is never stale, which the private fields and the `Arc` are what enforce.
- **Where it comes from, which depends on the producer:** loaded through `image::load`, it is the
  XXH3-64 of the whole file and equals hashing that path independently — the equality a file listing
  relies on, stated as the loader's guarantee where it belongs. Computed from other pixels, it is
  derived from the identity of what they were computed from together with what was done to them.

This is a correction, not a widening of what the type promises. `image-io`'s identity requirement
already opens with *"A loaded image SHALL…"* and closes with *"SHALL NOT be silently carried onto
different pixels"*, so the specs had this right and only the doc comment had generalised from one
producer. `Picture::new`'s warning that the caller owns the pairing is unchanged and becomes the place
a derived identity is constructed.

### D3. A rename, performed as a rename

`git mv` for the file, then the identifier changed at its 25 sites, and the existing tests run
unmodified apart from the name. No accompanying tidy-up, no signature adjusted "while we are here", no
field made public because it would be convenient for the next slice.

That discipline is what makes the change reviewable: a diff in which every hunk is either the old name
becoming the new one or a paragraph of prose about identity can be read for what it is, and a
behavioural change hidden in it would stand out. A rename bundled with improvements cannot be checked
that way, which is how a "pure refactor" becomes the change that broke something.

No deprecated alias is left behind. A `pub use SourceImage = Picture` would be the right courtesy for
a published crate with downstream users; this crate has none — it is not on any registry, and the only
consumer of the name is an integration test in this repository. An alias would be two names for one
type, forever, to spare nobody.

### D4. Platform and provider validation

Nothing here is platform-conditional, and nothing in it touches a runtime, a provider, a GPU, a model
file or the network. The existing `image-io` suite runs identically and in full on the ubuntu, macOS
and Windows CI jobs and is the whole of the verification: it already covers the identity equality, the
whole-file hashing, the shared-not-copied clone, the derived dimensions and the load/save/probe
contract, and every one of those tests passes unchanged except where it spells the type's name.

If any test needs more than its name changed, this change has done something it said it would not, and
that is the signal to stop rather than to adjust the test.

## Risks / Trade-offs

**A rename that silently drags a behaviour change with it** → The mitigation is D3's discipline plus
the shape of the evidence: the `image-io` suite is substantial and predates this change, so a green
run of tests nobody touched except to respell an identifier is a strong statement. The review question
is "does any hunk do anything other than rename or reword?", which a reader can actually answer.

**Churn against an archived capability for a naming reason** → Real, and the argument for paying it
now rather than never is that the cost only rises: one producer and one internal consumer today,
against two producers, a cache, a Tauri command and a React layer within three changes. Naming debt on
a public type is not the kind that stays the same size.

**`Picture` is a slightly unusual choice for a Rust image type** → `Image` is the conventional name and
is the one this cannot use (D1). The mitigation is that `Picture` is not invented here — it is already
the specs' word, used in the requirements a reader of this type will be reading alongside it.

## Migration Plan

Nothing on disk changes, no format moves, and no data is read or written differently. The change is
source-level only.

Consumers to update: `crates/opai/tests/public_api.rs`, which names the type as a front end would.
That is the complete list outside `src/`.

Reverting is the same rename in the other direction, with the identity documentation reverting to its
current, narrower wording — which is safe only for as long as no derived identity exists, so a revert
after `add-inference-5-kyoto` lands would need the documentation half kept.

**Ordering:** this change lands before `add-inference-5-kyoto`, which is written against `Picture`. If
the order is reversed, that slice carries the rename itself and this change becomes empty.
