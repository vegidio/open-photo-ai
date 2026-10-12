## Context

Everything the dialog shows is already reachable (see proposal.md - Why):

- `appVersion()` in `frontend/ipc/app.ts` invokes the `version` command, which answers `opai::version()`. The navbar
  calls it once on mount and keeps the result in local state.
- `once()` in `frontend/ipc/once.ts` keeps the promise of a fetch whose value cannot change while the process runs;
  `catalogue.ts` and `images.ts` already use it.
- `DialogTitleBar` draws the 48px title bar with a close box, and its documentation already names About as a caller.
- `navbar.about` and `navbar.about_dialog.{iconAlt,version}` are in all thirteen locales.
- `tauri-plugin-opener` is a dependency, used through its Rust API only; `capabilities/default.json` grants the
  frontend none of its permissions and says why.
- The reference's dialog is `cmd/gui/frontend/src/features/navbar/DialogAbout/index.tsx`, its icon
  `src/assets/icon.avif`, and its copyright and link constants `src/utils/constants.ts`.

## Goals / Non-Goals

**Goals:**

- Screen 19 as the design draws it, opened from the navbar's existing About button.
- Links that the webview cannot redirect: the frontend names *which* link, never the address.

**Non-Goals:**

- The "Update Available" button and the update check behind it.
- Analytics. The reference tracks nothing on About either.
- A Tauri window of its own. The dialog is an in-page modal, as Settings is.

## Decisions

### The links are opened by a closed command: `open_link(link: Link)`

A new `crates/gui/src/links.rs` holds `enum Link { Repository, Website }`, deserialized from `"repository"` /
`"website"`, each arm mapping to its URL; and the command `open_link`, which calls
`tauri_plugin_opener::open_url(link.url(), None::<&str>)`. The frontend's `frontend/ipc/links.ts` exposes
`openLink(link: "repository" | "website")`.

- *Why not grant `opener:allow-open-url` and call the plugin from JS:* it would reverse the recorded decision in
  `capabilities/default.json` that the frontend is granted nothing from the opener, and the frontend would hold a
  permission to open URLs where it only needs two named pages. Both paths end in the same `open::that_detached`, so
  the user sees no difference.
- *Why not a generic `open_url(url: String)`:* a command that opens any address the webview sends is the permission
  above without the scope that limits it.
- *Why a new module rather than `reveal.rs`:* `reveal.rs` exists for one platform problem - the file-manager reveal must
  run on a particular thread - that opening a URL does not share: `open_url` spawns a detached process and is safe
  from any thread. So the command is a plain synchronous `#[tauri::command]`, with no thread hand-off.
- Only the URLs move into Rust. The labels ("Github", "vinicius.io") stay in the frontend beside the copyright line,
  since they are what is drawn. The command and the frontend name the links with the same
  two strings, pinned by a test on each side, as `app.ts` and `logs.ts` pin their command names.

Errors: `open_url` failing (no handler registered for `https`) crosses as `LinksError::OpenLink { message }`, the
serialisable `#[serde(tag = "kind")]` shape `LogsError` uses.

### A failed link is logged, not toasted

The reference calls `Browser.OpenURL` without handling its promise, so a failure is silent. This change matches that:
the dialog `console.error`s a rejection and draws nothing. A toast would need a new `errors.*` key in thirteen locales
for a failure that only a machine with no default browser can produce. This is the one place the change could have
diverged and chose not to.

### The version is fetched once, through `once()`

`appVersion` becomes `once(() => invoke<string>("version")).get` - the version is fixed for the life of the process,
which is exactly what `once` is for. The navbar and the dialog both call it; the second call awaits the first's
promise. No store is introduced for one immutable string.

The dialog renders its version line only once the value has arrived, as the navbar does, rather than behind a
placeholder. In practice it has always arrived: the navbar asked at mount, long before anyone presses About.

### The dialog is `features/navbar/AboutDialog.tsx`, open state held by the navbar

The navbar keeps `const [aboutOpen, setAboutOpen] = useState(false)` and renders `<AboutDialog open onOpenChange />`
beside the button. Unlike Settings there is no draft, so no hook like `useSettingsDialog` is needed. It is built on the
shadcn `dialog` with `DialogTitleBar` (close box shown), so Escape, the backdrop and the close box are all Radix's
`onOpenChange` - the reference's dialog can be dismissed all three ways too.

`DialogContent` takes `aria-describedby={undefined}`, as Settings, Crop, Select faces and the TensorRT dialog do: the
dialog has no prose to describe it, and this tells Radix so instead of leaving it to warn.

### Layout follows screen 19

384px wide (`w-96`). Under the title bar, a column centred with a 16px gap and `10px 24px 28px` padding:

- the icon at 144px square (`size-36`), `alt` from `navbar.about_dialog.iconAlt`;
- `APP_NAME` at 20px bold, and beneath it, 4px apart, `navbar.about_dialog.version` at 13px muted;
- 6px further down, the copyright at 13px muted, and under it, 6px apart, the two links at 13px separated by a 12px
  vertical rule.

Link colour is the design's `#38bdf8` with `#7dd3fc` and an underline on hover. `style.css` has no token for it yet, so
one is added there rather than written as a raw hex in the component.

The links are `<button type="button">`s styled as links, not `<a href>`s: an anchor with an `href` in a Tauri webview
navigates the application window when it is clicked in any way the handler does not intercept (middle click, a
keyboard modifier), and `href="#"` as the reference uses scrolls and changes the location.

### Selectable text is opted into, not assumed

`style.css` sets `user-select: none` for the whole window, the way desktop chrome behaves. The version line and the copyright
line get `select-text`, as the reference's do; the name, the links and the icon stay unselectable.

### The copyright and link labels are constants in `lib/constants.ts`

`APP_COPYRIGHT = "© 2025—2026, Vinicius Egidio"` and `APP_LINKS = { repository: "Github", website: "vinicius.io" }`,
beside `APP_NAME` and for the reason `APP_NAME` gives: a name and a notice read the same in every language. Copied as
the reference writes them, em dash included.

### Crate division

`gui` only. `links.rs` and the command are GUI plumbing: a CLI has no link to click. `opai` gains nothing.

`rust-sak`: no module covers opening a URL in the user's browser, and none should - it is a desktop-shell concern
already solved by the Tauri plugin in the dependency tree. No new crate is introduced.

## Risks / Trade-offs

- [The URLs live in Rust while their labels live in TypeScript] → Both are pinned by tests naming the same two keys,
  and neither is likely to change; the alternative was a frontend permission, which was rejected above.
- [A machine with no default `https` handler] → The command rejects and the console says why; the dialog stays open.
  Same as the reference.
- [`once()` makes `appVersion` a cached value] → The version cannot change while the process runs; `once` clears a
  rejection, so an IPC failure on first call is retried on the next.

## Platforms

Designed for macOS, Windows and Linux; the opener's `open_url` has an implementation on each. To be validated by hand on
macOS only. Windows and Linux are untested for the link opening, which is the only platform-specific behaviour here.
No execution provider is involved.
