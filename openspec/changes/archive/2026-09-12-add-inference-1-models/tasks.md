## 1. The published manifest and its three sources

- [x] 1.1 Add `deps/model/manifest.rs` with the `serde` type one published file resolves to - name, size and SHA-256 - and the parse of the Hugging Face tree listing into it, reducing each entry's `path` to its basename and taking the hash from the LFS object id; verify tests over a committed recorded response cover a nested path reduced to its basename, and that an entry carrying no LFS object id is dropped rather than admitted with an empty hash (design.md - D2)
- [x] 1.2 Follow the listing to exhaustion rather than passing a large bound, so no fixed model count exists in the project; verify a test serves a listing spanning two responses through `deps/test_server.rs` and asserts an artifact named only in the second resolves as readily as one named in the first
- [x] 1.2a Close the `rust-sak` gap 1.2 rests on: `fetch` returns only response bodies, and the Hugging Face tree endpoint advertises its next page in a `Link` header. Add a `Response<T>` carrying body, status and headers, the four `*_response` methods that return one, and `Response::link(rel)` for the RFC 8288 parse - in the `rust-sak` repository, with its own tests and README - then tag it and bump this crate's pin; verify `rust-sak`'s suite is green and `opai` builds against the tagged version with no path override left behind (design.md - D6)
- [x] 1.3 Write the manifest to a cached copy atomically after a successful read and fall back to it after a failed one; verify tests cover the round trip, a half-written or undecodable cache treated as absent rather than as an error, and an empty cache treated as absent
- [x] 1.4 Add the manifest compiled into the binary - a checked-in JSON in the same shape as 1.1, `include_str!`'d and parsed once - and verify a test asserts it parses through 1.1's deserializer and that every artifact it names is a name `models/` can produce, without asserting the converse, since a nameable artifact need not be published (design.md - D3)
- [x] 1.5 Add the generator that produces 1.4's file from the live listing, as a script run by hand rather than a `build.rs`, and document when to re-run it; verify the script's output for the committed recorded response is byte-identical to what is checked in, so the generator and the parser cannot drift
- [x] 1.6 Add the resolution over the three sources in order - live, cached, compiled-in - refusing with an error naming the artifact where none of them has a hash for it, memoized for the process so later installs reuse it; verify tests cover all four outcomes and that two concurrent first installs read the listing once

## 2. From an artifact name to an installable dependency

- [x] 2.1 Resolve an `ArtifactId` to the files backing it by scanning the manifest for the `<id>.onnx` prefix, and to its own directory under `models/`; verify tests cover a single-file model, a model published as a graph plus a separate weights file, and two artifacts sharing a leading portion where neither takes the other's files (design.md - D1, D8)
- [x] 2.2 Make `Dependency`'s `name` and `dir` owned so a descriptor can be built from an `ArtifactId` rather than only from a `const` row, leaving the pinned table `&'static`; verify every existing `release.rs` and `artifact.rs` test passes unchanged, including each pinned platform's URL and destination

## 3. Installing a bare file

- [x] 3.1 Add the branch through `deps::install` that skips expansion for a descriptor whose sources need none, leaving the fingerprint skip, the resume, the single hash-mismatch retry and the manifest bookkeeping shared; verify a test installs a bare file and asserts it lands unexpanded, is recorded in the manifest, and that a second install of the same descriptor transfers nothing (design.md - D5)
- [x] 3.2 Add the model variant to `progress::Dependency` and confirm an install with no expansion phase spreads the whole 0-1 range over the transfer; verify tests assert the variant's user-facing name, that the fraction never decreases across a resumed transfer, that the final report is exactly `1.0`, and that an install with no recipient reports nothing (design.md - D7)
- [x] 3.3 Add the `pub(crate)` model install entry point and its error variants in `error.rs` - no published hash for this artifact, and a hash mismatch distinguishable from a transfer failure; verify tests assert an artifact absent from every source fails naming the artifact with no transfer attempted, and that a served file whose bytes are wrong fails as a mismatch

## 4. End to end against the test server

- [x] 4.1 Extend the test server suite to drive model installs; verify a model installs and reports success, a second install of an intact model transfers nothing, an install interrupted after some files are written is not mistaken for complete on the next attempt, replacing a model whose published file set changed removes only that model's previous files, two models installed in turn both remain present and usable, and that `Opai::initialize` reads the listing on no path of its own (design.md - D4)

## 5. Verification

- [x] 5.1 Document the capability at module level - the three manifest sources and the refusal that replaces the reference implementation's unverified install, the per-model directory, and that nothing here opens a graph - and verify `cargo doc -p opai` emits no warnings
- [x] 5.2 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked`, and verify all three are green on the ubuntu, macos and windows CI jobs
