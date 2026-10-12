## 1. The link command

- [x] 1.1 Add `crates/gui/src/links.rs` with `enum Link { Repository, Website }` (serde `rename_all = "camelCase"`),
      `Link::url()`, `LinksError::OpenLink { message }` tagged like `LogsError`, and a synchronous `#[tauri::command]
      open_link(link: Link)` calling `tauri_plugin_opener::open_url(link.url(), None::<&str>)`; register it in
      `generate_handler![]`; verify unit tests assert each link's URL, that `"repository"` and `"website"` deserialize
      and any other string is rejected, and the error's serialized shape
- [x] 1.2 Add `frontend/ipc/links.ts` exporting `openLink(link: "repository" | "website")`, with the command name
      written once and a comment pointing at `links.rs`; verify `links.test.ts` pins the command name and the argument
      it sends for each link

## 2. Shared pieces

- [x] 2.1 Make `appVersion` in `frontend/ipc/app.ts` a `once()` fetch; verify `app.test.ts` asserts two calls make one
      `invoke` and a rejected first call is retried by the second
- [x] 2.2 Add `APP_COPYRIGHT` and `APP_LINKS` to `lib/constants.ts` as the reference writes them, add a link colour
      token (`#38bdf8`, hover `#7dd3fc`) to `style.css`, and copy `icon.avif` from
      `../open-photo-ai/cmd/gui/frontend/src/assets/` into `frontend/assets/`; verify `pnpm typecheck` passes and
      `pnpm build` emits the asset

## 3. The dialog

- [x] 3.1 Build `features/navbar/AboutDialog.tsx` on the shadcn `dialog` and `DialogTitleBar` to screen 19's layout
      (see design.md), reading `navbar.about` and `navbar.about_dialog.*` - no new i18n keys, so no locale edits - with
      the version and copyright lines `select-text`, and each link a `<button type="button">` calling `openLink` and
      `console.error`ing a rejection; verify tests assert the icon's alt, the name, the version, the copyright, that
      each link sends its own name, that a rejected open leaves the dialog open, and that Escape and the close control
      dismiss it
- [x] 3.2 Wire the navbar's About button to open it, holding the open state in `Navbar`, and remove "About renders and
      does nothing" from its documentation and the comment above the button; verify `Navbar.test.tsx` asserts pressing
      About opens the dialog with and without an image open

## 4. Checks

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test` for the workspace; verify all three are clean
- [x] 4.2 Run `pnpm typecheck`, the Vitest suite, and biome scoped to the files this change touched; verify all three
      are clean
- [x] 4.3 Hand the user a launch check on macOS: About opens screen 19, the version and copyright select and copy, and
      both links open in the default browser while the window keeps the dialog; verify the user confirms it
