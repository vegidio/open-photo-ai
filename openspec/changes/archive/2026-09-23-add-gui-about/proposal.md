## Why

Screen 19 of the design, "About", is drawn in the navbar as a button that does nothing: the navbar's own comment says
"About renders and does nothing". The reference opens a small dialog from it naming the application, its version and
its author, with links to the repository and the author's site - and the version there is what a user copies into a bug
report.

Now because everything it needs has landed: the version already crosses the IPC boundary for the navbar, the dialog
title bar was written with About named as one of its callers, the dialog's copy is already translated in every locale,
and the opener plugin is already a dependency.

## What Changes

- **The navbar's About button opens the About dialog** drawn in screen 19: the application icon, the application name,
  the running build's version, the copyright line, and two links - Github and vinicius.io - separated by a rule.
- **The version and the copyright line are selectable**, so they can be copied into a bug report.
- **The two links open in the system's default browser**, never inside the application window. They are opened by a
  new Tauri command that accepts only a closed set of named links; the frontend cannot ask it to open any other
  address.
- **The icon asset** is brought over from the reference.
- **The version is fetched once** and read by both the navbar and the dialog.

Out of scope: the navbar's "Update Available" button, which screen 19 also draws. It depends on an update check `opai`
does not offer, and stays absent.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-shell`: a requirement is added - the navbar's About control opens a dialog identifying the application, its
  version and its author, whose two links open in the system browser.

## Impact

**Crates: `gui` only** - the frontend, plus one Tauri command. `opai` and `cli` are untouched. The command is a front
end's concern and not shared logic: opening a web page in the user's browser from a clickable link is something only a
windowed front end does. The CLI has no About surface, and the only thing the two front ends share here, the version,
is already `opai::version()`.

**rust-sak: nothing used and nothing missing.** Opening a URL is not one of its modules; `tauri-plugin-opener`, already
a dependency for revealing files, provides it.

**New dependencies: none**, crate or npm. No capability is granted: the plugin is reached through its Rust API from a
command of this application's own, as the log reveal already is.

**i18n: no new keys.** `navbar.about` and `navbar.about_dialog.{iconAlt,version}` are already in every locale. The
copyright line and the link labels are not translated, as in the reference.

**Task estimate: nine**, so one change rather than a series.

**Parity: no deliberate divergence in behaviour.** The reference opens both links in the system browser from the
frontend (`Browser.OpenURL`); this change does it from Rust through a closed command, which the user cannot tell apart.
