## Context

See proposal.md - Why. What this change builds on:

- `crates/gui/src/links.rs` holds `enum Link { Repository, Website }` and the synchronous `open_link` command, which
  opens only the address a `Link` maps to. `frontend/ipc/links.ts` writes the same names a second time, and a test on
  each side pins them.
- `Navbar.tsx` renders the version span with `mr-2` and a comment explaining that the button after it is absent.
  Screen 01 draws that button as `height:28px; padding:0 12px; margin-left:4px; font-size:13px`, white on the accent
  `#009aff`, radius 6px.
- `navbar.updateAvailable` is in every locale.
- The reference: `AppService.IsOutdated` (`cmd/gui/services/app_service.go`) returns
  `github.IsOutdatedRelease(ctx, "vegidio", "open-photo-ai", shared.Version)`, a bare `bool`; the navbar calls it on
  mount, shows `<Button size='small' className='ml-1 animate-pulse'>` when true, and opens
  `https://github.com/vegidio/open-photo-ai/releases`.
- `../rust-sak/src/github`: `is_outdated_release(owner, repo, version) -> Result<bool, GithubError>`, async, on the
  `fetch` module's client. It validates `version` before sending anything, and compares the release's `tag_name`
  leniently (`v` optional, missing components zero, pre-release older than release). Unauthenticated: 60 requests an
  hour per IP.
- The workspace version is `26.9.0` (`opai::version()` is `CARGO_PKG_VERSION`); the latest release is `26.9.4`.

## Goals / Non-Goals

**Goals:**

- Screen 01's button, shown exactly when the reference would show it for the same version.
- Reuse the closed link command, so the window still names a page and never sends an address.

**Non-Goals:**

- Downloading or installing an update. The reference only links to the releases page; Tauri's updater plugin is a
  different feature with signing and hosting behind it.
- The analytics event (see proposal.md - Impact).
- Periodic re-checks. Once per launch, as the reference, which also keeps well inside the rate limit.
- Stamping the version at release time. This repository does not release.

## Decisions

### The check is a `gui` command, `is_outdated`, over `rust_sak::github`

A new `crates/gui/src/update.rs` holds the owner and repository constants and an async `#[tauri::command]
is_outdated() -> bool` that awaits `rust_sak::github::is_outdated_release(OWNER, REPO, opai::version())`.

- *Why `gui` and not `opai`:* the rule is "both front ends would need it", and only one does - the reference's CLI
  never checks, and its check lives in `cmd/gui/services`, not in the library. Moving it to `opai` would add public
  API with one caller.
- *Why async:* the call is network-bound and `rust-sak`'s client is async. Tauri runs async commands on its Tokio
  runtime, which is what `fetch` needs, so there is no thread hand-off and the window is never blocked.
- *Why `rust-sak` and nothing else:* it is the port of the exact go-sak function the reference calls, including the
  version comparison. No crate is introduced.

### A failure is `false` on the Rust side, logged there

The command returns `bool`, not `Result`: it maps `Err` to `false` after a `tracing::warn!` carrying the error. The
window has nothing to draw for a failed check, and the reference makes the same collapse in go-sak.

- *Why not reject and let the frontend log:* the frontend's `console.error` goes to the webview's console, which no
  user sends in a bug report; `tracing` goes to the log file that Settings' "open logs" reveals. `false` is also the
  answer either way, so a rejection would be a second path to the same result.
- `Ok(false)` is not logged: being current is not an event.

The frontend still `console.error`s a rejection, which only a broken IPC bridge can produce - the same guard the
reference added after an unhandled rejection hid its button.

### The releases page is a third `Link`

`Link::Releases` maps to `https://github.com/vegidio/open-photo-ai/releases` and crosses as `"releases"`. Its URL is
built on the repository's, so the two cannot drift: `Repository`'s address becomes a `repository!()` macro both arms
use - a macro rather than a `const` because `concat!` takes only literals. `frontend/ipc/links.ts` widens `Link` to `"repository" | "website" | "releases"`, and both tests gain the name.

- *Why not let `update.rs` open the page itself:* then there would be two routes from the window to the browser. One
  closed enum is the whole of what the window can ask for, which is what the About requirement promises.
- *Why not return the release's `html_url` from the check:* the window would then hold an address to open, which is
  the permission `links.rs` exists to refuse. The reference opens the releases list, not a release, and so does this.

A failed open is `console.error`ed and changes nothing, as the About dialog's links do.

### The button: shadcn `Button`, `size="sm"`, `animate-pulse`

Rendered after the version span only once `isOutdated()` resolves `true`; absent before and otherwise. The default
(primary) variant is the accent on white, which is what screen 01 draws. `ml-1` is the design's `margin-left:4px`.
`animate-pulse` is the reference's: the design is a still, and cannot say whether the button moves.

The check is asked for once, from the navbar's mount effect, and held in the navbar's local state - one boolean with
one reader needs no store. `frontend/ipc/app.ts` gains `isOutdated()` wrapped in `once()`, as `catalogue.ts` and
`images.ts` wrap theirs, so a remount (React's strict mode in development) does not spend a second request of the
hourly sixty. The command never rejects for a failed check, so `once()` keeps a `false` for the launch - which is the
spec's "not retried".

The version span keeps `mr-2`, and the comment that explained it in terms of an absent button now explains it in terms
of the present one.

### Every build checks

Decided with the user: no `cfg!(debug_assertions)` gate. The reference's debug builds only skip because
`"<version>"` fails to parse, which is an accident of how Go stamps versions, not a rule. Here a debug build is the
only place the button can be seen working, and nothing ships from this repository.

### Tagging `rust-sak`

`github` is on `rust-sak`'s `main` but in no tag. Following the repository's own pattern (`chore: Bump the version to
26.9.7`, then the tag): bump its `Cargo.toml` version to `26.9.8`, commit, tag `26.9.8`, push the commit and the tag.
The user asked for this to be done here, and confirms the push at apply time.

Then every pin moves to `26.9.8` - `opai`, `gui` and `perf` - and `gui`'s feature list gains `github`. Before pinning,
`git ls-remote --tags` confirms the tag is on the remote, and a cold `CARGO_HOME=$(mktemp -d) cargo fetch --locked`
proves it resolves without the local git cache. The lockfile is refreshed with `cargo metadata` rather than
`cargo update -p rust-sak`, which has re-resolved unrelated packages here before.

### Crate division

`gui`: `update.rs`, the `Link::Releases` arm, the frontend. `opai`: the pin only. `cli`: nothing. `perf`: the pin only.

`rust-sak` features used: `github` (new, in `gui`), which enables `fetch` - already built for `opai`, so this adds no
compile of `reqwest`.

## Risks / Trade-offs

- [The button is lit on every launch until the workspace version passes the latest release] → Intended; see "Every
  build checks". It also points at the Go app's releases, which is right for as long as the rewrite lives here.
- [Unauthenticated GitHub requests are capped at 60 an hour per IP] → Once per launch, and `once()` stops remounts
  spending more. A user behind a shared NAT who hits the cap sees no button, which is the fail-safe direction.
- [Tagging `rust-sak` is a release other projects can see] → The commit is already on `main`; the tag only names it.
  The push is confirmed at apply time.
- [A `rust-sak` pin that resolves only from the local cache] → The cold fetch above; it has bitten this project before.

## Validation

macOS only (the development machine). Windows and Linux are untested; nothing here is platform-specific beyond
`open_url`, which the About dialog already exercises on each. No execution provider is involved.
