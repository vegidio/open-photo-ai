## 1. The crop in `gui`

- [x] 1.1 Pin `rust-sak` in `crates/gui/Cargo.toml` at the tag `opai` and `perf` already use, with
      exactly the `image` and `crypto` features (design.md D13), documenting in the manifest comment
      that a third pin of one git source must move in lockstep with the other two or the lock file grows
      a second `rust-sak` and `DynamicImage` becomes two unrelated types. Verify `cargo build --locked`
      resolves with `Cargo.lock` still holding exactly one `rust-sak` entry, and that `cargo tree` shows
      no crate compiled on this pin's account that was not already in the graph.
- [x] 1.2 Add `crates/gui/src/images/crop.rs` with the `Crop` value from design.md D1 - the two flips,
      the rotation as an `i32` count of millidegrees with `rotation_degrees()` beside it, and the
      rectangle in the rotated picture's coordinate space - deriving `Copy`, `Eq` and `Hash` because it
      is half of two cache keys, with `Crop::new` refusing a zero-width or zero-height rectangle and
      `Crop::scaled` for D5's reduced coordinate space. Verify the type is `Eq + Hash` by using one as a
      `HashMap` key in a test, that a zero-sized rectangle is refused, and that `scaled` leaves the flips
      and the angle alone while scaling the rectangle.
- [x] 1.3 Add the composition - `cropped(&Picture, Crop) -> Picture`, blocking, since both callers are
      already on blocking threads - flipping horizontally, then vertically, then rotating through
      `rust_sak::image::rotate` at `rotation_degrees()`, then cutting with `DynamicImage::crop_imm` - the
      methods rather than the `image::imageops` free functions of the same names, which would flatten a
      sixteen-bit source (D13) -
      with the rectangle clamped into the rotated bounds and floored at one pixel per edge (D6), and
      pair the framed pixels with the identity D3 derives - `rust_sak::crypto::xxh3_string` over the
      source identity, a `crop` discriminator and every field of the crop, handed to
      `opai::Picture::new` with the source's own path - answering the source picture unchanged,
      identity included, for a crop that changes nothing. One task because it is one function: pixels
      that arrive without the identity that describes them are the invariant `Picture` exists to hold.
      Verify tests cover the order being flip-flip-rotate-cut against a picture that distinguishes all
      four, a positive angle turning clockwise, a rectangle running past the edge being clamped rather
      than failing, a rectangle wholly outside answering the nearest pixel that exists, a 16-bit source
      staying 16-bit, a rotated corner inside the rectangle being transparent, a cropped picture not
      carrying its source's identity, two crops of one picture differing, the same crop twice agreeing,
      and the no-op crop answering the source untouched in identity and pixel buffer both.

## 2. The crop on the way to the window

- [x] 2.1 Give `Asked` its `crop: Option<Crop>` and teach `parse` the `crop=` parameter from design.md
      D4 - `<left>,<top>,<width>,<height>,<millidegrees>,<flips>` with `<flips>` one of `-`, `h`, `v`,
      `hv` - refusing the whole request when it does not parse in full, the way a malformed `size`
      already is. Rewrite the type's "left room for without being named yet" comment now that it is
      named. Verify tests cover a URL with no `crop` parsing exactly as it does today, a well-formed
      crop round-tripping every field including a negative angle, each malformed form being refused
      rather than read as no crop, and both platform forms of the URL parsing identically.
- [x] 2.2 Apply the crop in `render` with D5's reduce-first rule: the factor
      `bound / max(crop.width, crop.height)` capped at 1, the source reduced through the existing
      `bounded`, the crop scaled by the reduction actually achieved, then framed through
      `crop.rs`'s `cropped`. Verify tests cover an unbounded cropped request being served at
      exactly the rectangle's dimensions, a bounded cropped request landing within a pixel of the bound
      on its longest edge and showing the same framing as the unbounded one, a crop of a rotated picture
      being served as PNG because it carries alpha, and an uncropped request being byte-identical to
      what it is today.
- [x] 2.3 Pin the cache behaviour the crop inherits from `Renditions` being keyed on `Asked`: verify
      tests cover one identity, bound and crop being rendered once and served twice, two different crops
      of one identity and bound being two entries neither of which serves the other's framing, and a
      crop-bearing request for an identity the application never admitted being refused exactly as an
      uncropped one is.

## 3. The crop on the way to a run

- [x] 3.1 Carry the crop on the `enhance` command and apply it in `enhance_with` where D7 puts it -
      after the slot is written and the source is loaded, before `Opai::process` - so the chain, every
      per-operation cache key and the result's identity are all the framed picture's. Verify tests cover
      a run over a framed image reporting the framing's enhanced dimensions, two framings of one
      photograph answering different identities, the same chain and crop twice answering the same
      identity, a request carrying no crop behaving exactly as it does today, and a chain fed a rotated
      (and therefore alpha-bearing) picture running rather than being refused.
- [x] 3.2 Add the crop to `ipc/enhance.ts`'s `Operation`-side wire shape - the `crop` argument on
      `enhance`, typed from the shared `CropInfo` - and verify `enhance.test.ts` pins the argument's
      name and shape, that omitting it sends no crop, and that the Rust side's field names match.

## 4. The crop in the window

- [x] 4.1 Add `ipc/crop.ts` with the `CropInfo` type and the one function that spells it for a URL
      (D8), and `stores/crop.ts` - `Map<identity, CropInfo>` with its reader hook, registered as a file
      owner so closing an image forgets its crop and closing all of them clears the map, not persisted,
      and documented as having no writer until slice 3. Verify tests cover the query spelling
      round-tripping against the Rust parser's grammar, a crop surviving a move between images, closing
      one image leaving another's crop alone, and closing every image clearing it.
- [x] 4.2 Carry the crop on the rendition URL: `renditionUrl` and `renditionFor` take one, the canvas
      and `useSettledFile` pass the current image's, the sidebar's miniature passes it (D5's cost
      argument is what makes that affordable), and `DrawerItem` deliberately does not (D12). Verify
      tests cover a URL with no crop being unchanged, a crop appearing in the query, the miniature and
      the canvas asking for the same framing, the strip asking for none, and the settled-file preload
      waiting on the *cropped* URL so the canvas swaps pixels and box together.
- [x] 4.3 Add the helper that answers a record's dimensions under a crop (D9) and size the canvas's pane
      from it, so the box is the framing's shape before the pixels arrive. Verify tests cover a pane
      published at the crop's dimensions rather than the file's, an uncropped image being unchanged, and
      the fitted photograph keeping the framing's proportions on a canvas of another shape.
- [x] 4.4 Make the navbar report the effective size and compare the three on hover (D11): the block
      shows the output dimensions when the stack enlarges the image and otherwise the framing's, falling
      back to the file's, and the popover - built from the existing `components/ui/popover.tsx` - shows
      Original, Cropped (only when a crop is set) and Output. Put the upscale-factor derivation in
      `lib/enhancements.ts` beside the other derivations of that shape. Verify tests cover the block for
      an uncropped image, a cropped one, and one with an upscale; the popover's three rows and the
      cropped row's absence without a crop; and the whole block staying absent with no image open and
      for an image the application could not measure.
- [x] 4.5 Make the crop part of what a run is: add it to the run effect's dependencies and to the held
      result's pairing, so changing a crop stops the run in flight, starts one over the new framing and
      releases the previous result rather than drawing it to the new shape (D10). Verify tests cover a
      crop change starting a new run, the held result disappearing on that change while an enhancement
      change still keeps its result, a crop change on an image with no enhancements asking for no run,
      and the previous result's pixels being let go of by the run that replaces it - asking for a run
      is what displaces whatever the backend was holding, enforced there rather than by this path calling
      the release command, which stays the stack store's to call when a chain is emptied or an image is
      closed.

## 5. Checks

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test
      --locked`, and the frontend lint, typecheck and test scripts scoped to the files this slice
      touched (the repository-wide formatter run rewrites files this slice never opened). Verify all of
      them pass, that the i18n parity test passes with no new keys added - `navbar.dimensions.original`,
      `.cropped` and `.output` are already in all thirteen locales - and that `openspec validate
      --changes add-crop-2-seam --strict` passes.
- [x] 5.2 Verify by hand that an uncropped application is unchanged: open a photograph, check the canvas,
      the miniature, the strip and the navbar draw and read exactly as before, run an enhancement, and
      confirm the rendition URLs in the devtools network panel carry no `crop` parameter. This slice
      ships no way to set a crop, so "nothing changed" is its user-visible acceptance criterion.
