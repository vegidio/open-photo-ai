## 1. `opai` - the operations the seam needs

- [x] 1.1 Add `image::resize` and `image::resize_blocking` beside `load`/`save`/`probe`: bound the
      longest edge with Lanczos3, preserve the aspect ratio, and return the picture unchanged when the
      bound is at or above its longest edge (D2). Verify with tests covering a landscape image, a
      portrait image, a bound larger than the image, and that the asynchronous and blocking forms
      agree - the shape `image/mod.rs`'s existing round-trip tests already use.
- [x] 1.2 Add `image::input_extensions`, deriving the list from an exhaustive match over
      `ImageFormat` plus `RAW_EXTENSIONS` filtered by the existing `decodable` check, so a ninth
      format is a compile error rather than a silent omission (D5). Verify with a test asserting the
      eight encodable extensions are present, that all 29 decodable RAW extensions are, and that none
      of the 18 refused ones is.
- [x] 1.3 Publish the two operations the seam needs and the crate kept to itself, so `gui` never names
      `rust-sak` in its own manifest (D9): `image::identity`/`identity_blocking`, the XXH3-64 of a
      file's bytes with no decode, and `image::encode`/`encode_blocking`, `save` with no file at the
      end of it, through the same encode dispatch, plus the pathless `ImageIoError::Encode` its
      failure crosses as. Verify the identity equals the loaded picture's for a standard format and a
      camera RAW, covers trailing bytes and answers for a file that is not an image; and that the
      encode's bytes equal what saving the same picture writes, that all eight formats round-trip,
      that mismatched options are refused, and that a 16-bit picture is narrowed for JPEG rather than
      refused.

## 2. `gui` - Rust

- [x] 2.1 Add `tauri-plugin-dialog` to `crates/gui/Cargo.toml` and register it in the builder,
      carrying a manifest comment in the style of the `opener` and `os` entries that states why no
      npm package and no capability entry accompany it (D4). Verify the crate builds and
      `capabilities/default.json` is unchanged.
- [x] 2.2 Add the image record - path, identity, width, height, lowercase extension, byte size - and
      the function that describes a batch of paths: concurrent on blocking threads, tolerant per file,
      ordered by path (D8). Verify with tests covering a sound file, a file whose header cannot be
      read surviving alongside sound ones, a missing file, and the same selection described twice
      giving the same order.
- [x] 2.3 Add the opened-file registry to Tauri's managed state, written only by the two commands and
      read only by the protocol handler (D4). Verify with tests that an unadmitted identity resolves
      to "unknown" and an admitted one whose file has been removed resolves to "unreadable", as the
      spec requires those two to be distinguishable.
- [x] 2.4 Add the `open_images` command: `async` so the plugin's blocking picker runs off the main
      thread (R1), filtered from `image::input_extensions`, taking its title and filter wording from
      the frontend, admitting what it returns, and reporting a dismissal as no files rather than a
      failure. Verify by opening and cancelling the picker on macOS.
- [x] 2.5 Add the `describe_images` command for paths supplied by any other route, funnelling into
      the same describe-and-admit path as 2.4. Verify with a test that the record for one file is
      equal whichever of the two commands produced it.
- [x] 2.6 Register the `opai` asynchronous URI scheme and parse its requests into an identity and a
      bound, treating an absent or zero `size` as the image's own dimensions (D1, D3). Verify with
      tests over the request forms, including a malformed one and a request naming a location rather
      than an identity.
- [x] 2.7 Implement the handler: resolve through the registry, `load`, `resize` to the bound, encode
      as JPEG quality 90 or as PNG when the decoded picture carries an alpha channel (D7), and stream
      it with an immutable cache directive (D6). Verify with tests covering a bounded rendition, an
      unbounded one, a transparent source coming back with its transparency, and the unknown and
      unreadable responses being distinguishable.
- [x] 2.8 Add both platform forms of the scheme to `img-src` in `tauri.conf.json`'s `csp` and
      `devCsp`, and extend the drift guard in `src/lib.rs` so the test fails if the scheme registered
      in Rust is absent from either (R4). Verify the new assertion fails when the scheme name is
      changed in one place only.

## 3. `gui` - frontend

- [x] 3.1 Add `ipc/images.ts`: typed wrappers over the two commands, and a rendition-URL helper built
      with `convertFileSrc(identity, "opai")` so neither platform's URL form is written by hand (D3).
      Verify with tests over the helper's output and the command wrappers, in the style of the
      existing `ipc/*.test.ts`.
- [x] 3.2 Confirm no catalogue work is needed: `dialogs.native.selectImage` and
      `dialogs.native.imagesFilter` are the only strings this slice uses and both already exist in all
      thirteen locales. Verify `i18n/parity.test.ts` still passes and no key was added.

## 4. Verification

- [x] 4.1 Run the seam against `~/Desktop/test`: describe its seven files, confirm each reported
      identity equals the file's XXH3-64 computed independently and each reported dimension pair
      matches the file, then fetch one rendition bounded at 100 and one unbounded and confirm both
      decode and carry the expected sizes.
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace, and
      `pnpm lint`, `pnpm typecheck` and `pnpm test` in `crates/gui`. Verify all six are clean.
