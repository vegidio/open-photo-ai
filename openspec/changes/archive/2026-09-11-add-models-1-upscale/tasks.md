## 1. Shared model vocabulary

- [x] 1.1 Add `Precision` (`Fp32`/`Fp16`/`Int8`) with its lower-case rendering, plus the per-contract
      `ConvPrecision` (FP32, FP16) and `OsakaPrecision` (FP16, INT8) and their infallible widening
      into `Precision`. Verify with tests that each renders the string its filenames use, and that no
      construction path produces an Osaka FP32 - per spec *Upscale variants and their precisions*.
- [x] 1.2 Add the `Scale` type bounded to 1-8, quantized to three decimal places on acceptance and
      stored as an integer in milli-units so it derives `Eq` and `Hash` soundly, with `new` returning
      `Result` and `clamped` infallible. Verify with tests that 0.5 and 12 are refused by one and
      bounded by the other, that 1.66661 and 1.66669 both become 1.667 and compare equal, and that
      every accepted scale compares equal to itself - per spec *Upscale scale range* and *A scale
      compares exactly*.
- [x] 1.3 Add the `Family` prefix enum covering all eight two-letter prefixes, and the `ArtifactId`
      newtype built from a family, codename, optional native pass scale and precision. Do **not** add
      an operation-id type: an operation is its own identity. Verify with tests that each family
      renders its documented prefix, that an artifact name is composed from the native pass scale and
      never from a requested one, and that a whole native scale renders `_4x_` not `_4.0x_` - per spec
      *An artifact name is built from the native pass scale*.
- [x] 1.4 Add `serde` as a direct `opai` dependency and derive serialization on the public model types,
      with deserialization routed through the same validating constructors as direct construction.
      Verify with tests that an operation round-trips to an equal value and that a payload pairing
      Osaka with FP32, or carrying a scale of 12, is refused - per spec *Operations survive a round
      trip through serialization*.

## 2. Upscale variants

- [x] 2.1 Add the three convolutional variants (Tokyo, Kyoto, Saitama) as rows carrying codename,
      label and scale-bucket table - Kyoto covering 2x/4x/4x+2x, Tokyo and Saitama 4x/4x+4x. Verify
      with a test that each row's buckets are ordered and that their bounds reach 8x with no gap.
- [x] 2.2 Add Osaka as the diffusion variant: three graphs addressed by the roles encoder, transformer
      and decoder, with the two VAE halves pinning FP16 while the transformer follows the operation.
      Verify with tests that the VAE rows stay FP16 under an INT8 operation and that requesting an
      undeclared role returns an error naming it - per spec *Osaka resolves to its three named graphs*.
- [x] 2.3 Derive `PartialEq`, `Eq` and `Hash` on the operation types so an operation is usable as a
      registry key directly, and implement the display name for both contracts - `Kyoto 4x (FP16)`
      carrying the scale, `Osaka (INT8)` omitting it. Verify with tests that two identical requests
      compare equal and hash alike, that two convolutional scales and two precisions do not, that two
      Osaka scales do, and that both display names match - per spec *An operation is its own identity*
      and *Models report a display name*.

## 3. Artifact resolution

- [x] 3.1 Implement convolutional resolution: requested scale to an ordered pass sequence via the
      bucket table, each pass naming its artifact, plus the deduplicated set of required artifacts.
      Verify with tests covering Kyoto at 1.5x, Kyoto at 8x (two different artifacts), Tokyo at 8x
      (one artifact required once for two passes), and that no accepted scale yields an empty
      sequence - per spec *Convolutional upscalers resolve to a pass sequence*.
- [x] 3.2 Implement Osaka resolution to its three role-addressed artifacts. Verify with tests that
      FP16 requires the three `_fp16` stems and INT8 requires `up_osaka_int8` alongside the unchanged
      `up_osaka_vae_encoder_fp16` and `up_osaka_vae_decoder_fp16`, and that artifact ids are stems
      carrying no file extension - per spec *Osaka resolves to its three named graphs* and the design's
      *An artifact id is a stem, not a filename*.
- [x] 3.3 Implement the per-run parameter set and the image cache tag, the tag written explicitly
      rather than derived from the serde output. Verify with tests that two Osaka operations that
      compare equal still report different tags for different scales, that a repeated operation
      reports a stable tag, and that no two operations producing different images share one - per spec
      *Every operation reports an image cache tag*.

## 4. Catalogue and public surface

- [x] 4.1 Add the catalogue, built from the same variant rows and bounds constants that drive
      resolution rather than restated beside them. Verify with tests that it reports all four variants
      with their labels, the correct precisions for each, and scale bounds that equal what `Scale`
      enforces - per spec *The catalogue publishes variants and parameter ranges*.
- [x] 4.2 Re-export the public model types from the crate root alongside the existing exports, and
      document the module. Verify that the existing public API (`Opai`, `InitError`,
      `SupportedProviders`, `prepare_library_path`, the progress types) is unchanged and that
      `cargo doc` builds with no broken intra-doc links.

## 5. Quality gate

- [x] 5.1 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, and
      verify all three pass clean from the repository root.
