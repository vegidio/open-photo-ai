## Why

The batch series lets `opai` write many files. Slice 3 gives it `--format` (including Preserve), `--prefix`,
`--suffix`, numbering by default, and `--overwrite`. The GUI's export dialog has these rules already, but they are
split between `crates/gui` and its frontend: `export/format.rs` (which formats exist and what Preserve writes),
`export/destination.rs` (claiming a free `_1`..`_999` name, and overwriting through a hidden file renamed over the
target), and `lib/export.ts` (composing `prefix + stem + suffix + extension` and choosing the folder). By the
project's rule, logic both front ends need belongs in `opai`. This slice moves it there before the CLI needs it, so
the CLI gets the rule from `opai` instead of a third copy.

## What Changes

Task estimate: **14**. This change is slice 2 of the four-slice `cli-batch` series (see design.md - Slices).

**In `opai`:**

- **A new public module, `opai::export`**, holding the export naming rules both front ends use:
  - **Formats.** The eight writable formats in the GUI's spelling (`heic`, `jpeg`, …), a *format choice* that is
    either one of them or **Preserve**, each format's extension (`jpg` for JPEG), the capability table, and the
    encoder settings for a format at a quality. Moved from `gui`'s `export/format.rs`.
  - **Names.** For a source path and a naming (prefix, suffix, format choice), it gives the file name, the format
    written and the extension. Given a folder, or none (meaning the source's own directory), it gives the
    destination path. Preserve keeps the source's own extension, lowercased, where that format can be written, and
    writes TIFF (`.tiff`) otherwise, which covers camera RAW and files with no extension. Moved from the frontend's
    `formatFor`, `extensionFor`, `exportNameFor` and `destinationFor`.
  - **Claiming a destination.** Without overwriting it takes the first free name among `name`, `name_1` …
    `name_999`, created exclusively. With overwriting it writes through a hidden name beside the target and renames
    that over the target on commit. Unless committed, the claim gives its name back. Moved unchanged from `gui`'s
    `export/destination.rs`.

**In `gui` (no change in behaviour):**

- `export/format.rs` and `export/destination.rs` are deleted. The `export` command and `export_formats` use
  `opai::export`.
- **Wire change (internal, not user-facing):** the `export` command no longer takes `destination` and `format`.
  It takes the export settings' naming instead (`prefix`, `suffix`, optional `location`, format choice) and works out
  the destination from the source's path in `opai`.
- **A new command, `export_names`**: given the queued paths and the naming, it returns each file's name, destination,
  format and extension. The window uses it to show each row's name and its type change, and to choose the quality
  format at Save.

**In the frontend (no change in what the user sees):**

- `formatFor`, `extensionFor`, `exportNameFor` and `destinationFor` are removed from `lib/export.ts`, together with
  their path joining. Rows show the names Rust returns, asked for again when the prefix, suffix, folder or format
  changes. `qualityFor`/`queueQualityFor` read each file's format from the same answer. A write failure names the
  path given in the error.

## Capabilities

### New Capabilities

- `export-naming`: the library's rule for naming an export, shared by every front end: the format choice and
  Preserve's fallback, the extension each format is written with, composing the name and destination, claiming a free
  numbered name, and overwriting only on commit.

### Modified Capabilities

None. `gui-export`'s requirements (the row's name, the extension rules, numbering, overwriting, nothing left behind)
read the same after this change. Only where the rule is computed moves. `cli-enhance` changes in slices 3 and 4.

## Impact

**Crates touched:** `opai` (new `export` module, public API), `gui` (the export command and its wire, the new
`export_names` command, two files deleted) and the frontend (`lib/export.ts`, `features/export/batch.ts`,
`ExportRow`, `ipc/export.ts` and their tests). No logic stays in a front end: the GUI keeps only the Tauri commands,
the wire types and the display. `cli` is not touched. It adopts `opai::export` in slice 3.

**`rust-sak`:** checked `fs`. It has temporary files (`mk_temp_file_in`, `TempHandle`), but nothing that claims a
free numbered name or writes through a hidden file and renames it over the target. Its temporary files are created
readable by their owner only, which the current claim already avoids for that reason (an export renamed from one
would keep those permissions). Nothing is added to `rust-sak`. See design.md for why the claim stays in `opai`.

**Third-party crates introduced:** none. `opai` already depends on `serde`, and `tempfile` is already one of its
dev-dependencies.

**Not in scope:**

- Any CLI change, flag or spec delta (slices 3 and 4).
- Changing a naming rule. For example, numbering, the `_999` limit, Preserve's TIFF fallback and the lowercase
  extension stay exactly as they are today.
- Changing the shape of `export_formats`'s answer. The window stops reading `preserves` and `fallback` from it, but
  the fields are kept (see design.md).
