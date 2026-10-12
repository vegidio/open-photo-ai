## Context

See `proposal.md` - Why. What matters here is why the obvious shapes are wrong and this one is not.

Two shapes have already been considered and rejected in archived designs, and neither rejection is
being relitigated:

- **One generic `Enhancement { family, variant, amount }`** - rejected in
  `add-models-2-enhancements/design.md`, on four grounds: the family becomes data rather than a type;
  nothing stops a denoise codename being paired with the sharpen family; colorization carries an
  `Option` that is structurally always `None`; and a family that later diverges must either widen the
  shared shape for everyone or be lifted out.
- **A macro or a generic `Operation<V, P>` collapsing the family modules** - rejected in
  `refactor-models-family-layout/design.md`, because the per-family doc comments carry most of the
  reasoning in those modules and a macro would vaporise them.

What no design has considered is the third option: **keep the eight distinct types, and add one seam
over them.** That is this change. It is worth being explicit that it contradicts neither rejection:

| The enhancement design's objection | Does it apply here? |
|---|---|
| The family becomes data instead of a type | No. Each arm holds a distinct type; `Operation::Denoise` can only hold a `Denoise`. |
| A denoise codename could pair with the sharpen family | No. The pairing is still guaranteed by construction inside each arm. |
| Colorization carries an always-`None` `Option` | No. `Colorization` is unchanged and still carries no parameter field. |
| A diverging family must widen the shared shape | No. A family that diverges grows its own type, as upscale and face recovery already have; only the forwarding arm changes. |

And the layout design's objection - that a macro destroys the doc comments - does not apply because
no family module is rewritten. All eight keep every line they have.

## Goals / Non-Goals

**Goals:**

- One type over the eight families, so that adding a ninth is a compile error rather than a silently
  skipped test row.
- The catalogue's inverse shipped rather than living in `#[cfg(test)]`, so the round trip the
  catalogue exists to guarantee is available to the front end it exists for.
- The cross-family invariants expressed once, over `Family::ALL` and `Operation`, rather than over
  hand-transcribed lists and magic counters.

**Non-Goals:**

- Collapsing any family module, or `Scale`/`Strength`/`Bias`. Explicitly out of scope; see Context.
- Changing any existing type, signature, cache tag, artifact name or doc comment. This change is
  purely additive to the public surface.
- Generalising `upscale/resolve.rs`. Pass sequences, scale buckets and graph roles are genuinely
  upscale's own; hoisting them would be over-generalisation. `Operation::required_artifacts()`
  forwards to each family's existing method and asks nothing more of them.

## Decisions

### An enum, not a trait

A trait would need `dyn` or a generic parameter at every call site, and the thing being modelled is a
**closed** set - the library knows all eight families, and a consumer cannot add one. An enum makes
that closure explicit, keeps `Operation` `Serialize`/`Deserialize` across the IPC boundary that
`models/mod.rs` opens by describing, and makes the exhaustive match the module doc already promises
into something that exists.

`#[serde(tag = "family")]` gives the front end a discriminated union it can build a chooser from and
send back, which is the shape the process boundary wants.

### `from_codename` on the variant enums, not one central map

The catalogue is already built per family from each family's `ALL` rows. Putting the inverse beside
those rows keeps one source of truth per family and makes the central `Operation` constructor a
one-line arm per family rather than a second twenty-row table. It also means a family that adds a
variant updates exactly one file.

### What the counters become

`checked == 40` and `checked == 5` are standing in for "every published row was exercised". With
`Family::ALL` and a shipped inverse, the assertion becomes a traversal - every family in
`Family::ALL` appears in the catalogue, and every catalogue row builds - and the literals go. A test
that counts is a test that must be edited when the domain grows; a test that traverses is not.

## Risks / Trade-offs

- **`Operation` is a second way to name an operation.** A consumer can hold a `Denoise` or an
  `Operation::Denoise`. This is deliberate - the typed form stays the one you build, and `Operation`
  is what you store, send, or dispatch on - but it must be documented on both, or a reader will not
  know which to reach for.
- **Forwarding is boilerplate.** Eight arms per forwarded method, six methods. It is mechanical and
  the compiler checks it is complete, which is the whole point; but it should not quietly grow a
  method per family need. Anything only one family answers stays on that family's type.
- **Serde representation is a commitment.** Once a front end persists an `Operation`, the tag and
  field names are a compatibility surface. Pin them in the spec and in a round-trip test from the
  start, as the existing per-family serialization tests already do.

## Migration

None required. Every change is additive: no existing type, signature or re-export moves, so nothing
downstream can break. `tests/public_api.rs` gains assertions for the new surface rather than edits to
existing ones.

## Platforms

Pure vocabulary code with no platform-conditional paths, no native calls and no execution-provider
involvement. Validated the same way on all three targets by `cargo test`; nothing here is
platform-specific and nothing is left untested on any target.
