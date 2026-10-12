## 1. The primitives the dialog is built from

- [x] 1.1 Run `shadcn add select sonner` in `crates/gui`, apply the hand-edits every `shadcn add` here needs - the
      bogus `cn` package removed and its import repointed, arrow functions, named React imports - plus the one this
      pair adds, `sonner.tsx` dropping `next-themes` and pinning the theme because dark is forced app-wide, then
      mount one `<Toaster />` in `frontend/providers.tsx` beside the existing two providers and never inside a
      feature; verify `pnpm check` and `pnpm typecheck` pass, `git diff package.json` shows `sonner` and nothing
      else added, and a test raising a toast asserts the message appears once
- [x] 1.2 Add `components/ui/dialog-title-bar.tsx` - the design's 48px bar with its title and close box, shared
      because crop, export, faces and About all draw it; verify a test asserts it labels the dialog for assistive
      technology and that its close control calls back

## 2. What the preferences are, and where they live

- [x] 2.1 Add `frontend/i18n/languages.ts` as a leaf importing nothing: the thirteen tags, their endonyms, the
      supported guard, and detection that walks the environment's ordered preference list and matches on the base
      tag; verify tests cover a shipped language, an unshipped one falling back to English, a regional tag and an
      underscore-separated tag, and that the parity test now asserts this list and `CATALOGUES`' keys agree
- [x] 2.2 Add `frontend/stores/settings.ts` - language, analytics, processor, per-family model selections and
      per-format export qualities, persisted under `settings-storage` with the wire's own value spellings, repairing
      on rehydrate an unshipped language, a quality that is absent, zero or out of bounds, and a processor not among
      those offered - then make `frontend/i18n/index.ts` read the persisted language instead of its fixed `lng`,
      replacing the comment that says this store has not been written; verify tests assert each setter writes only
      its own field, that each broken stored shape reads as its repair while a sound value beside it is untouched,
      and that a seeded language is the one the application comes up in
- [x] 2.3 Give the store its snapshot and restore, both driven by one `satisfies`-checked list of its data keys,
      copying arrays and records rather than referencing them; verify a test changes every field, restores, and
      asserts all of them reverted - and that a data field left out of the list fails to compile

## 3. Reading the catalogue as choices

- [x] 3.1 Add `frontend/lib/enhancements.ts` naming the seven enhancement families in pipeline order with their name
      and description keys - detection named nowhere - and the resolver over it that turns a family's catalogue entry
      plus a stored selection into the options to offer and the one selected, every variant at every precision it
      publishes with tiers read positionally through `models.label`, absent-or-unpublishable falling back to the
      first variant at its first precision; verify tests assert every family listed is one the catalogue publishes
      and that there are seven, and cover a family never chosen for, a stored model the library no longer publishes,
      a stored precision a published model no longer offers, Osaka's own pair of precisions, and a
      single-precision variant getting no tier suffix

## 4. The dialog

- [x] 4.1 Add the settings section table - the four sections and the fifteen ids under them, read by both panes -
      and the dialog frame over it: the card at the design's metrics, the title bar, the nav column, the scrolling
      section pane and the Cancel/Save footer; verify tests assert every table entry has a row and every row an
      entry, that a click on a nav entry scrolls its row into view and highlights it, and that a click outside
      neither closes nor discards
- [x] 4.2 Wire the navbar's Settings button to open it, opening the draft as it opens - which records what Cancel
      restores to and closes storage for writing - and make Cancel restore while Save applies, including
      `changeLanguage` on save and not before; verify tests assert the button opens it, that cancelling several
      changed rows reverts all of them with the language never applied, that saving a language changes the
      application's text, and that nothing reaches storage until Save so a draft cannot outlive a process that
      never got to either button
- [x] 4.3 Add the four row shapes - select, switch, button and slider - each drawing label, control and description
      as the design does, the slider carrying its default mark and its value; verify a test per shape asserts it
      reports its change and renders its description

## 5. The fifteen rows

- [x] 5.1 The Application rows: Language over the endonyms, Logs calling slice 6's two commands with a failed reveal
      reported as a toast carrying `errors.showLogsFailed` in the running language rather than passing silently, and
      Analytics persisting and nothing more; verify tests assert the languages are listed in themselves whatever
      language is active, that Show logs resolves the path and reveals it, that a rejected reveal shows the message,
      and that turning analytics off sends nothing
- [x] 5.2 The Performance row: the processor built from the setup store's report - automatic first, the reported
      providers, then the CPU - with the chosen provider as the description's i18next context; verify tests assert a
      GPU-offering report lists that provider, a CPU-only report lists two choices, that an unknown fifth field in
      the report is not offered, and that the description follows the selection
- [x] 5.3 The seven Enhancement rows, through the resolver, each titled by its family's own name key; verify a test
      asserts all seven render with options from the catalogue, that detection has no row, and that choosing a model
      writes that family's selection alone
- [x] 5.4 The four Export rows, one per lossy format, each with its own starting value and mark; verify a test
      asserts the four starting values, that moving one leaves the other three untouched, and that the control
      cannot be driven outside the accepted bounds

## 6. Verification

- [x] 6.1 Run the frontend `lint`, `typecheck` and `test` scripts; verify all three pass
- [x] 6.2 Launch the application on macOS, open Settings from the navbar and compare it against screens 16 and 16b -
      the card's metrics, the nav column, the subheader bands, the slider marks and the footer; verify the processor
      list matches what this machine reported, that Show logs opens Finder with the log selected, that picking a
      language changes the application only after Save, and that Cancel leaves everything as it was
