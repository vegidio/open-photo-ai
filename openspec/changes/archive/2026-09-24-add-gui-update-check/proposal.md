## Why

Screen 01 of the design draws an "Update Available" button at the navbar's right edge, after the version. It has
never been rendered: `add-gui-2-shell-empty` left it out because the check behind it was a network call nothing here
offered, and `add-gui-about` kept it out for the same reason. The reference checks GitHub once per launch and, when a
newer release exists, shows a pulsing button that opens the releases page - how a user running a superseded build
finds out.

Now because the check exists: `rust-sak` gained a `github` module with `is_outdated_release`, the port of the go-sak
function the reference calls. It only needs a tag to be pinned by, and the closed link command `add-gui-about` wrote
already opens pages in the browser.

## What Changes

- **The application checks once per launch whether a newer release exists** on `vegidio/open-photo-ai`, comparing the
  running build's version (`opai::version()`) against the latest published release's tag. This repository is a
  temporary home for the rewrite, so the releases it compares against are the reference's own.
- **When one does, the navbar shows "Update Available"** after the version, pulsing as the reference's does. It opens
  the project's releases page in the system's default browser.
- **A failed check shows nothing.** No network, a rate limit or an unparseable tag all read as "no newer release
  known", and the failure is logged.
- **The link command gains a third page**, the releases page, alongside the repository and the website.
- **`rust-sak` is tagged** at the commit that adds `github`, and every pin of it in the workspace moves to that tag.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-shell`: a requirement is added - the navbar offers an update when a newer release is published - and the About
  dialog's links requirement is widened from "only those two pages" to the closed set of pages the application links
  to, which now includes the releases page.

## Impact

**Crates: `gui` only**, plus the `rust-sak` pin in `opai` and `perf`. The check is a front end's concern, not shared
logic: the reference makes it from `cmd/gui/services` and its CLI never checks, so `gui` is the only caller, and a
button to press is the only thing the answer is for. If the CLI ever wants it, it moves to `opai` then.

**rust-sak: found, but untagged.** `github::is_outdated_release(owner, repo, version)` is exactly the check, with the
same lenient semver comparison go-sak makes. It landed in `3b6ed87`, one commit after `26.9.7`, and no tag contains it
locally or on the remote. This change cuts the next tag in `../rust-sak` and pushes it; the go-ahead for that push is
given at apply time, since a push there is a release. All three pins (`opai`, `gui`, `perf`) move together: two tags of
the same git source are two crates to Cargo, and `gui/Cargo.toml` already records what two copies of `rust-sak` break.

**New dependencies: none.** `gui` gains `rust-sak`'s `github` feature, which pulls `fetch` (already built for `opai`),
`semver`, `serde` and `thiserror`.

**i18n: no new keys.** `navbar.updateAvailable` is already in every locale.

**Task estimate: nine**, so one change rather than a series.

**Parity - three deliberate divergences:**

- The reference tracks an `UpdateAvailable` analytics event when the check says yes. This change does not: the
  application has an analytics setting but nothing that sends events, and adding a sender for one event is not this
  change's subject.
- Debug builds check too. In the reference they never show the button, but only because its unstamped version
  (`"<version>"`) fails to parse; here the version is always the workspace's, and a debug build is the only way to see
  the button work. Until the workspace version reaches the latest release, the button is lit on every launch.
- A failed open is logged rather than toasted, as `add-gui-about` decided for its own links. The reference toasts
  `errors.openUrlFailed`, a key this application does not carry.
