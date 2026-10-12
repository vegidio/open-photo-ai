## Context

See `proposal.md` - Why. What matters here is the shape of what exists.

`crates/opai/src/models/` holds two categories of module with no visible seam between them. Shared
vocabulary - `artifact.rs`, `precision.rs`, `scale.rs`, `strength.rs`, `bias.rs`, `simple.rs` - sits
alphabetically interleaved with the families that consume it, and `upscale/` is a directory while
the other five families are flat files:

```
models/                              code  tests
+-- mod.rs                             63    86   re-exports + cross-family tests
+-- catalogue.rs                      122   316
+-- artifact.rs  precision.rs  scale.rs  strength.rs  bias.rs  simple.rs
+-- denoise.rs                        187   246
+-- sharpen.rs                        186   246
+-- light_adjustment.rs               181   248
+-- color_balance.rs                  183   254
+-- colorization.rs                   140   215
+-- upscale/ mod.rs  variant.rs  resolve.rs    (556 code, 638 tests)
```

Three constraints shape the approach:

- **The public surface cannot move.** Everything is re-exported flat: family module -> `models/mod.rs`
  -> `lib.rs` -> `opai::{Upscale, GraphRole, Pass, ...}`. Nothing outside `models/` names a submodule
  path, and `tests/public_api.rs` imports from the crate root exactly as a consumer does.
- **Each family already has the same internal seam.** Every family file is the same five sections in
  the same order: variant rows, variant enum, operation, params, identity impls. `upscale/` already
  splits on that seam; the flat files only imply it.
- **`upscale` is genuinely bigger.** 556 lines of code against ~185 per simple family. It is the only
  family with two contracts, the only one whose parameter is part of the model's identity, and the
  only one resolving to more than one artifact. Any rule has to accommodate that without either
  forcing it flat or making it the template.

## Slices

Not a slice of anything. This is a single self-contained change, estimated at 11 tasks (see
`proposal.md` - Impact), well under the 15-task limit.

It sits downstream of the completed `add-models-1-upscale` / `add-models-2-enhancements` series,
which is archived and finished. The layout rule this change establishes is what the two families
that series deliberately left open - Face Recovery and Detection - will be built against when they
are proposed.

## Crate split

**`opai` only.** No `gui` or `cli` work and none possible: these are domain types both front ends
read through the crate root, none of them names a `models::` submodule path, and no logic moves
toward or away from a front end. Nothing here is logic at all - it is the same code in different
files.

## rust-sak and other crates

**Neither is touched.** This change writes no infrastructure code - no hashing, HTTP, retries,
downloads, archive extraction, temp files, config directories, image codecs, caching, telemetry or
hardware probes - so no `rust-sak` module is relevant, no feature is enabled or added, and no gap in
it is exposed. No third-party crate is introduced, and no dependency in any `Cargo.toml` changes.

## Decisions

### A family is a directory; `mod.rs` is the operation, `variant.rs` is what can be named

Every family gets the same two modules:

```
models/
+-- mod.rs  catalogue.rs                        cross-family
+-- artifact.rs  precision.rs  scale.rs         shared vocabulary:
+-- strength.rs  bias.rs  simple.rs             flat files, unchanged
|
+-- denoise/
|   +-- mod.rs        Denoise, DenoiseParams, Display/PartialEq/Hash
|   +-- variant.rs    DenoiseVariant + STOCKHOLM/GOTHENBURG/MALMO + ALL
+-- sharpen/          mod.rs + variant.rs
+-- light_adjustment/ mod.rs + variant.rs
+-- color_balance/    mod.rs + variant.rs
+-- colorization/     mod.rs + variant.rs
+-- upscale/
    +-- mod.rs        Upscale, UpscaleParams, identity impls
    +-- variant.rs    UpscaleVariant + Conv/Diffusion rows + GraphRole
    +-- resolve.rs    Pass, GraphSet, Resolution
```

Stated as three rules:

1. A family is a **directory**. `mod.rs` is the operation; `variant.rs` is what can be named. A
   family adds further modules only where its own contract needs one.
2. A **file** directly under `models/` is shared vocabulary or cross-family code, never one family's.
3. Tests split on the same seam - variant-level in `variant.rs`, operation-level in `mod.rs`.

The split is not arbitrary: it follows the seam every family already has between *naming* a model
and *running* one. `variant.rs` answers "what can be asked for" - the rows, the enum, the codename
and label and precision. `mod.rs` answers "what one run is" - identity, parameters, cache tag,
display name. Rule 2 is what makes rule 1 legible from `ls` alone: directories are families, files
are everything else.

**Alternative - a directory per family holding a single `mod.rs`** (rename only, no splitting). It
is the smallest possible change and makes the *directories* uniform, but opening `denoise/` and
`upscale/` still shows different things, so it buys no predictability - which is the entire point.
Rejected.

**Alternative - flatten `upscale` back into one file.** 556 lines of code plus 638 of tests in one
module, against ~185 + ~250 for a simple family. Rejected: it makes the largest, most irregular
family the least navigable one to make the directory listing tidy.

**Alternative - a `shared/` directory for the vocabulary,** so the rule reads "every directory is a
family except `shared/`". Marginally crisper to state, but it deepens every family's imports for a
separation that rule 2 already delivers, and `upscale/` already reaches `super::super::artifact`.
Rejected on cost.

### `mod.rs` rather than the `foo.rs` + `foo/` form

The 2018-edition idiom would be `denoise.rs` beside a `denoise/` directory, avoiding six files named
`mod.rs`. This project uses `mod.rs` throughout - `models/mod.rs`, `deps/mod.rs`, `upscale/mod.rs` -
and consistency with the crate it lives in outweighs the editor-tab cost. It also avoids reproducing
the exact visual pattern this change exists to remove: a `.rs` file sitting beside a directory of
the same name.

### Each family's `mod.rs` re-exports the rows the catalogue reads

`catalogue.rs` today reaches `denoise::ALL` at one level but `upscale::variant::{KYOTO, OSAKA,
SAITAMA, TOKYO}` at two. Once `ALL` moves into `denoise/variant.rs` that asymmetry would spread to
all six. Instead each family's `mod.rs` carries `pub(crate) use variant::ALL;` - and upscale's the
four rows by name - so the catalogue names every family at one level and does not know how a family
arranges itself internally.

Upscale keeps naming its four rows individually rather than gaining an `ALL`: its catalogue entry is
hand-built because Osaka's precision list differs from the other three, which is a real difference
and not boilerplate to be collapsed.

### `simple.rs` keeps its name

It holds `SimpleVariant` and `single_artifact` - the row type and composition shared by the five
single-graph families - so it is vocabulary, not a family, and rule 2 keeps it as a flat file.
Renaming it to `variant.rs` for symmetry would put a seventh `variant.rs` in the crate and, worse,
imply it belongs to a family. Left exactly as it is.

### `every_variant()` moves to `variant.rs` in all six families

Each family's `pub(crate) fn every_variant()` test helper is read by the cross-family tests in
`models/mod.rs`. It builds variants, so it belongs beside the enum - which means `variant.rs::tests`
for all six, including upscale, where it currently sits in `mod.rs`. The cross-family test reaches
it as `denoise::variant::tests::every_variant()`. Uniform, and the one place the test seam is
visible from outside a family.

### Nothing else is collapsed

The four parameterized families (`Denoise`, `Sharpen`, `LightAdjustment`, `ColorBalance`) are
~185 lines of near-identical code each, differing in the names and in `Strength` vs `Bias`. The
archived `add-models-2-enhancements` design accepted this deliberately, and a macro or a generic
`Operation<V, P>` would collapse it. Both are out of scope here and stay rejected for the reason
that design gave: the per-family types make dispatch an exhaustive match the compiler checks, and
the doc comments carry most of the reasoning in these modules - a macro would vaporise them. This
change moves code between files and changes no type, signature, or comment.

## Goals / Non-Goals

**Goals:**

- One shape for all six families, and a rule that decides the shape of the seventh and eighth.
- The shared vocabulary visibly separate from the families that consume it.
- The rule recorded where a later change will read it, not only in the code it describes.
- A pure move: every type, signature, doc comment, test and artifact name identical afterwards.

**Non-Goals:**

- Any behaviour change. No accepted value, refused value, artifact name, cache tag, display name or
  serialized form moves.
- Any public API change. No type is added, removed, renamed or re-exported differently.
- Collapsing the duplication between the four parameterized families - see Decisions.
- Adding Face Recovery or Detection. This change gives them a shape to be built in; they remain
  their own future changes.
- Reorganising anything outside `models/`. `deps/` has the same `mod.rs` convention and is not in
  scope.

## Where the rule is recorded

In the `context` block of `openspec/config.yaml`, under `# Conventions`, beside the existing "Tests
live beside the code they cover" bullet. That block is loaded into every change, which is the
property that makes the rule survive this change being archived.

**Not** in `openspec/specs/models/spec.md`. The project's own `specs` rule requires requirements to
be written against observable application behaviour rather than internal structure, and a file
layout is exactly the structure that rule excludes. A spec delta here would be inventing a
requirement to have somewhere to put a convention. `.openspec.yaml` is marked `skip_specs: true`
accordingly.

## Risks / Trade-offs

- **A "pure move" silently stops being pure** - a doc link retargeted, a test dropped in the split,
  a `pub(crate)` widened to `pub` → The compiler catches visibility and path errors, `cargo test`
  catches a test that stopped compiling, and `cargo doc` catches a broken intra-doc link. What none
  of them catches is a test silently *deleted* during the split, so each family's task states the
  test count before and after as the check.
- **The public surface moves without anyone noticing** → `tests/public_api.rs` imports every exported
  type from the crate root the way a consumer does. It is not modified by this change, so an
  incomplete or altered re-export fails there. Leaving that file untouched is itself the control.
- **Two files per family is ceremony for the smallest ones** - `colorization/variant.rs` is ~65 lines
  of code → Accepted. The alternative is the rule having exceptions, which reintroduces the problem;
  and the file is small because the family is small, not because the split is wrong.
- **Six files named `mod.rs`** → Accepted deliberately; see Decisions. Consistency with `deps/` and
  the rest of the crate was judged worth the editor-tab cost.
- **The rule lives in `config.yaml`, which nothing enforces mechanically** → True, and there is no
  mechanism that would. The mitigation is placement: it sits in the context block every change
  loads, next to the other conventions, rather than in a document a future change might not open.
- **Merge conflicts against work in flight in `models/`** → The working tree is clean and no other
  change is active (`openspec list` reports none), so the window is this change alone.

## Migration Plan

There is nothing to deploy and nothing to migrate: no on-disk format, no serialized form, no
artifact name and no cache tag is touched, so an installation from before this change is
indistinguishable from one after it.

Family by family, each in its own task, so the tree compiles and the suite passes between families
rather than only at the end. Rollback is `git revert` of a commit that changed no behaviour.

## Validation

`cargo fmt --check`, `cargo clippy` and `cargo test` on the developer's platform. No
platform-specific or execution-provider-specific code is touched - nothing in `models/` loads a
session, names a provider or reads the filesystem - so the usual macOS/Windows/Linux and
CPU/CoreML/CUDA/TensorRT matrix has no bearing on this change, and CI covers the rest.
