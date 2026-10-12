# Design — add-image-encode-depth

## Context

See proposal.md — Why. The measured state of the seam, taken from the encoders rather than assumed,
is the table below; it is the fact everything here is built on.

`rust-sak`'s `encode_into` hands a `DynamicImage` to one of eight codecs. Three routes exist today:
`image`'s own encoders (BMP, GIF, JPEG, PNG, TIFF) via `write_to`/`write_with_encoder`, and the
author's `avif-rs` / `heif-rs` / `webp-rs` encoders via `write_with_encoder`. Every one of them
accepts a fixed set of `ExtendedColorType`s and answers `ImageError::Unsupported(Color(..))` for the
rest.

The `image` crate has a hook for exactly this — `ImageEncoder::make_compatible_img`, called by
`DynamicImage::write_with_encoder_impl` before `write_image` — but only four of its codecs implement
it (BMP, JPEG, TGA, and its own unused WebP encoder), and the helper behind it,
`dynimage_conversion_8bit`, is `pub(crate)` and cannot be reused.

**What each encoder accepts, measured at `image` 0.25.10, `avif-rs`/`heif-rs`/`webp-rs` 26.7:**

| Format | Accepts | Gap |
|---|---|---|
| BMP | L8 La8 Rgb8 Rgba8 — **plus its own `make_compatible_img`** | none |
| JPEG | L8 Rgb8 — **plus its own `make_compatible_img`** | none |
| PNG | L8 La8 Rgb8 Rgba8 L16 La16 Rgb16 Rgba16 | Rgb32F Rgba32F |
| TIFF | L8 Rgb8 Rgba8 L16 Rgb16 Rgba16 Rgb32F Rgba32F | **La8 La16** |
| GIF | **Rgb8 Rgba8 only** | L8 La8 L16 La16 Rgb16 Rgba16 Rgb32F Rgba32F |
| AVIF | L8 La8 Rgb8 Rgba8 L16 La16 Rgb16 Rgba16 | Rgb32F Rgba32F |
| HEIF | L8 La8 Rgb8 Rgba8 L16 La16 Rgb16 Rgba16 | Rgb32F Rgba32F |
| WebP | L8 La8 Rgb8 Rgba8 (libwebp is 8-bit only) | L16 La16 Rgb16 Rgba16 Rgb32F Rgba32F |

Slice 3 reported two cells of this table (GIF and WebP refusing `Rgb16`) because that is what a
developed RAW hits. The other four gaps are reachable from files a user already loads — an 8-bit
grayscale PNG exported as GIF, a grayscale-with-alpha PNG exported as TIFF — and are closed here
because one mechanism closes all of them. The 32-float rows are unreachable today (nothing in this
project produces a 32-float image) and are covered anyway, so the table has no holes left to
rediscover.

## Slices

The RAW series, continuing the one `add-image-io` opened. **This change is slice 4, and the last.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-image-io` | Loading, writing and probing images over `rust-sak`; `SourceImage` and its identity; the RAW dispatch seam with nothing behind it | — | **done** (archived `2026-09-12-add-image-io`) |
| 2 | `add-image-raw-1-rust-sak` | RAW decoding in `rust-sak`'s `image` module behind a new `image-raw` feature: `zenraw` on its `rawler` backend, a decode-only `RawFormat`, a synthesised DNG fixture, and a `rust-sak` release. Here: the pin moved to that release with the feature enabled | 1 (for the seam it fills) | **done** (archived `2026-09-12-add-image-raw-1-rust-sak`) |
| 3 | `add-image-raw-2-adopt` | Filling the seam in `opai`: a RAW file loads instead of being refused, a RAW file can be described without being decoded, `UnsupportedRaw` retires for the 29 extensions the backend covers and stays for the 18 it does not | 2 | **done** (archived `2026-09-12-add-image-raw-2-adopt`) |
| 4 | `add-image-encode-depth` | A picture is converted to what the target encoder accepts instead of being refused for its colour type — in `rust-sak`'s encode dispatch, because it is general-purpose and affects every consumer — plus a release and the pin here | 3 (which makes 16-bit the normal case) | **CURRENT** |

Slice 4 closes the series. Its scope is wider than the row slice 3 wrote for it: that row named the
16-bit half, and the measurement above found four more colour-type gaps on the same dispatch, all
fixed by the same step. The widening is deliberate — see D2.

## Goals / Non-Goals

**Goals:**

- No picture the system can produce is refused by a supported encoder for its colour type.
- Precision and transparency survive wherever the target can hold them, so the fix does not quietly
  cost a 16-bit PNG export its depth in exchange for making a GIF export work.
- One mechanism, one place, one release — not a per-format workaround accumulating in `opai`.
- A picture an encoder already accepts takes exactly the path it takes today, byte for byte.

**Non-Goals:**

- Dithering, palette control or any quantisation quality tuning for GIF. The `image` crate's GIF
  encoder quantises internally at its own `speed` setting; that is its decision, unchanged here.
- Colour management. Narrowing 16 bits to 8 is an arithmetic shift, not a perceptual or gamma-aware
  conversion. Neither the Go application nor `rust-sak` does colour management today (recorded in
  slice 1), and this change does not open it.
- Extending any encoder to accept more colour types natively. libwebp is 8-bit; that is a property
  of the codec, not of the dispatch.
- Anything in `gui` or `cli`. Nothing calls `save` from a front end yet.

## How the work divides

**`rust-sak` for the mechanism, `opai` only for the pin and what it pins.** The conversion is
general-purpose: every `rust-sak` consumer that encodes a 16-bit picture hits it, so by the project's
own test it belongs there rather than here. `opai`'s share is a git-tag bump, the spec-pinning test
that currently asserts the refusal, and the module documentation that currently calls it a known
limitation.

No `gui` or `cli` work, and none deferred into one. Both front ends export images, so encoding is
shared logic and lives in `opai` — settled in slice 1, and nothing here reopens it.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Encode all eight formats | `encode_file`, `encode_writer`, `EncodeOptions` | `image` (already enabled) |
| Narrow a `DynamicImage` to a given colour type | `image` crate's `to_rgb8`/`to_rgba8`/`to_luma8`/`to_luma_alpha8`/`to_rgb16`/`to_rgba16`/`to_luma16`/`to_luma_alpha16` | — |
| **Choose which colour type each format needs** | **nothing — this is the gap** | — |

The one missing piece is the per-format choice. The `image` crate keeps its own version
(`dynimage_conversion_8bit`) `pub(crate)`, so it cannot be called; and it would be the wrong answer
anyway, since it narrows everything to 8 bits and PNG, TIFF, AVIF and HEIF can hold more.

**The gap is filled by extending `rust-sak`, not by code local to this project**, for the reason the
project's rule gives: it is general-purpose, it belongs to the module that owns the encoders, and
filling it locally would mean `opai` converting pixels before calling an API whose contract is to
accept a `DynamicImage`. **No third-party crate is introduced on either side** — the conversions are
`image`'s own methods, and `image` is already a dependency of both crates.

## Decisions

### D1 — One `make_compatible(image, format)` in `rust-sak`'s dispatch, not eight encoder hooks

A single function in `dispatch.rs` maps a `DynamicImage`'s colour type and a target `ImageFormat` to
the colour type to convert to, returning `Option<DynamicImage>` — `None` when no conversion is
needed, so the accepted case allocates nothing. `encode_into` calls it once, before the `match` that
selects the codec, and `encode_to_vec` inherits it by delegating.

*Alternatives considered.*

- **Implement `make_compatible_img` on `webp-rs`'s (and `avif-rs`'s, `heif-rs`'s) encoders.** The
  hook exists and `write_with_encoder` already calls it, so WebP would need no dispatch change at
  all, and every direct `webp-rs` consumer would benefit. Rejected as the primary mechanism because
  it cannot cover GIF, PNG or TIFF — those are third-party encoders in the `image` crate that
  `rust-sak` cannot implement a trait for — so the dispatch would need the step regardless, and the
  fix would end up split across two repositories and two releases for no gain in this project. It
  remains a reasonable future addition to `webp-rs` on its own merits; it is not this change.
- **Convert in `opai` before calling `save`.** Rejected by the project's rule and by the shape of the
  bug: `opai` would have to carry the same eight-format table, and the next `rust-sak` consumer would
  rediscover the gap.
- **Fork `image`'s `dynimage_conversion_8bit`.** It is `pub(crate)`, and narrowing to 8 bits
  unconditionally would silently cost PNG and TIFF exports their depth. See D2.

### D2 — Narrow to the *widest* type the target accepts, not to 8 bits

The rule is: pick the colour type the encoder accepts that loses the least — depth first, then alpha,
then grayscale. The resulting table:

| Source | BMP | JPEG | PNG | TIFF | GIF | AVIF | HEIF | WebP |
|---|---|---|---|---|---|---|---|---|
| L8 | — | — | — | — | **Rgb8** | — | — | — |
| La8 | — | — | — | **Rgba8** | **Rgba8** | — | — | — |
| Rgb8 / Rgba8 | — | — | — | — | — | — | — | — |
| L16 | — | — | — | — | **Rgb8** | — | — | **L8** |
| La16 | — | — | — | **Rgba16** | **Rgba8** | — | — | **La8** |
| Rgb16 | — | — | — | — | **Rgb8** | — | — | **Rgb8** |
| Rgba16 | — | — | — | — | **Rgba8** | — | — | **Rgba8** |
| Rgb32F | — | — | **Rgb16** | — | **Rgb8** | **Rgb16** | **Rgb16** | **Rgb8** |
| Rgba32F | — | — | **Rgba16** | — | **Rgba8** | **Rgba16** | **Rgba16** | **Rgba8** |

(— means the encoder takes it as-is and `make_compatible` returns `None`. BMP and JPEG are dashes
throughout because their own `make_compatible_img` runs afterwards and already handles everything;
duplicating it here would convert twice.)

Two rows are worth pointing at. `La16 → TIFF` becomes `Rgba16`, not `La8`: TIFF has no
grayscale-with-alpha at any depth, so alpha forces the move to RGBA, and there is no reason to give
up the depth as well. `Rgb32F → PNG` becomes `Rgb16`, not `Rgb8`, for the same reason — a
float-precision picture written to a format that holds 16 bits should get 16.

*Alternative considered.* Uniform narrowing to 8 bits, matching `image`'s own helper: half the code
and one rule to remember, but it would make this change a regression for the case it is named after —
a 16-bit RAW exported as PNG would come back 8-bit, which is worse than today's honest refusal on
GIF. Rejected outright.

### D3 — BMP and JPEG are left alone

Both implement `make_compatible_img`, which runs inside `write_with_encoder` *after* anything the
dispatch does. Converting first would mean two conversions for one encode, and for JPEG a worse one:
its hook sends `La8` to `to_luma8` (keeping grayscale) where a generic 8-bit narrowing would send it
to `to_luma_alpha8` and then its hook would narrow again. Leaving them out is both cheaper and more
faithful to what those encoders chose.

This is asserted rather than assumed: a test pins that BMP and JPEG still encode a 16-bit picture,
so a future `image` release that drops either hook fails loudly here instead of silently.

### D4 — The conversion is in `encode_into`, which makes it universal by construction

`encode_to_vec` delegates to `encode_into` for every format except BMP, GIF and TIFF, and for those
three it calls `DynamicImage::write_to`. Putting the step in `encode_into` alone would therefore miss
GIF — the single most important case. It goes at the top of both functions instead, or equivalently
`encode_to_vec`'s three seek-bound arms convert first; either way the invariant to hold is that no
codec is reached without the step having run. A test that writes every format through both
`encode_file` (streaming) and `encode_writer` over a non-seekable sink (buffering) covers both paths,
so a step added to one and forgotten in the other fails.

### D5 — `opai` changes only its pin, its assertions and its prose

No code change in `crates/opai/src/image/`. `save_blocking` passes the `DynamicImage` straight to
`encode_writer`; once `rust-sak` converts, the refusal is gone with no call-site change.

What does change:

- `crates/opai/Cargo.toml`: the `rust-sak` tag 26.9.3 → 26.9.4.
- `crates/opai/src/image/mod.rs`: the test
  `a_developed_raw_is_sixteen_bit_and_two_of_the_eight_encoders_refuse_it` is the spec-pinning test
  for the removed requirement. It becomes
  `a_developed_raw_is_sixteen_bit_and_every_encoder_writes_it` — the developed RAW is still asserted
  16-bit, and all eight formats now write and reload at the DNG fixture's dimensions. The
  `ImageIoError::Codec` / `Unsupported(Color(Rgb16))` branch goes.
- `crates/opai/src/image/mod.rs`: the module-documentation section "A developed RAW is 16-bit, and
  two encoders refuse that" is rewritten to say the conversion happens and what it costs (a WebP
  export of a RAW is 8-bit). That sentence is the one a caller needs; the limitation prose goes.

The new `opai`-level assertions for the non-RAW gaps (grayscale to GIF, grayscale-with-alpha to
TIFF) are deliberately **not** added here: they are `rust-sak`'s contract, tested there against all
eight formats, and duplicating them would put this crate in the business of re-testing its
dependency. What `opai` tests is what `opai` promises — that a picture it loaded can be saved in any
format it offers.

### D6 — Release and pin are separate tasks, in that order

`rust-sak` is published, so the sequence is: change it, test it, get the go-ahead, tag 26.9.4, then
move `opai`'s pin. The version follows the crate's `YY.M.patch` scheme and 26.9.3 is current, so
26.9.4 is next. This is the same boundary slices 2 and 3 were split along, except that here the
`rust-sak` half is small enough that the whole thing fits in one change under the task limit.

The pin is a git tag, so `opai` cannot see the fix until the tag exists. Nothing in `opai` is edited
before it does, which keeps the repository compiling at every step.

## Risks / Trade-offs

- **A user exports a 16-bit RAW as WebP and gets 8 bits without being told** → It is the only thing
  libwebp can hold, and the alternative is today's error. The module documentation says which formats
  keep 16 bits, so the information is where a caller looks. A UI that wants to warn has the facts to
  do it; nothing here prevents that.
- **Narrowing is a shift, not a colour-managed conversion** → Consistent with the rest of the
  project, which does no colour management anywhere (slice 1). It matches what the `image` crate's
  own BMP and JPEG hooks already do, so a picture's GIF export and its JPEG export narrow the same
  way rather than differently.
- **The scope is wider than the row slice 3 wrote** → Four extra table cells and roughly ten extra
  match arms, in the same function, covered by the same tests. The alternative is a slice 5 for the
  identical seam, which costs another release cycle for a fix that is already open in the editor.
- **A future `image` release changes what an encoder accepts** → The table goes stale silently, in
  the safe direction for a widened encoder (a redundant conversion) and the unsafe direction for a
  narrowed one (a refusal returns). The test that writes every colour type to every format is what
  catches it, and it is written to enumerate rather than sample, so a new refusal fails a test rather
  than reaching a user.
- **Converting twice by accident** → The D3 exclusion for BMP and JPEG is the only place this could
  happen, and it is pinned by the test named there.
- **`opai`'s pin moves before `rust-sak` is verified on all three platforms** → The `rust-sak` half is
  tested in its own repository's CI before the tag exists, and the ordering in D6 makes that the
  gate.

## Migration Plan

No data, no persisted state, no API change. The deployment order is D6's: `rust-sak` change → its
tests → tag 26.9.4 → `opai` pin → `opai` tests and prose. Rollback is reverting the pin to 26.9.3,
which restores the previous behaviour exactly, since nothing in `opai` depends on the new behaviour
beyond the assertions that describe it.

## Platforms and providers

Pure Rust, no platform-specific code, no `unsafe`: the conversions are `image` crate methods on a
`DynamicImage`. The same tests run on macOS, Windows and Linux in both repositories' CI. Nothing here
touches model sessions, memory budgets or execution providers, so there is no ownership or lifetime
model to describe and no provider to validate against.

One platform-shaped caveat carried from slice 1: the AVIF encoder hangs on sub-16px frames, so every
fixture that reaches an AVIF encode stays at or above 16x16. The existing
`no_fixture_is_small_enough_to_hang_the_avif_encoder` test guards this in `opai`; `rust-sak`'s own
encode tests already size their AVIF fixtures for it.

## Open Questions

- Whether `webp-rs` should implement `make_compatible_img` on its own account, so direct consumers
  outside `rust-sak` get the same conversion. Deferrable: it is additive, it changes nothing about
  this change's specs, approach or tasks, and `rust-sak`'s dispatch is correct either way — the hook
  would simply find nothing left to convert.
