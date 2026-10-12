## 1. Store and primitives

- [x] 1.1 Add `settingsDefaults()` to `stores/settings.ts` and build the store's initial state from it; add
      `autopilotExcluded: Family[]` to `SettingsData`, `settingsData` and the `merge` repair (non-array reads as `[]`,
      unknown families and duplicates dropped); verify tests assert that a first launch equals `settingsDefaults()`,
      that a stored state without the field reads `[]`, that each broken shape reads as its repair while sound fields
      beside it are untouched, and that two calls return records that are not shared
- [x] 1.2 Run `shadcn add input` in `crates/gui`, apply the six hand-edits every `shadcn add` here needs, and pin its
      fill to `background` over the generated `dark:bg-input/30`; verify `pnpm typecheck` passes, `git diff
      package.json` is empty, and biome is clean on the new file only

## 2. Copy

- [x] 2.1 Add, change and remove the `settings.*` keys listed in design.md decision 10 in all thirteen locale files,
      translating every new or changed string; remove `Enhancement.descriptionKey` from `lib/enhancements.ts`; verify
      the locale parity test passes and `pnpm typecheck` reports no removed key still being read

## 3. The frame

- [x] 3.1 Replace `sections.ts` with `pages.ts` (id, icon, title and description keys, and the `SettingsData` keys
      each page holds), including a type-level check that the pages' keys cover `keyof SettingsData` exactly; verify
      that leaving a store field off every page fails `typecheck`, and a test asserts no key is on two pages
- [x] 3.2 Rebuild `SettingsNav` as four icon entries with `aria-current="page"`, and `SettingsDialog` as a page pane
      (title, description, the active page's body, scrolling when it overflows) opening on General, with the design's
      184px nav on `#141417`; verify tests assert it opens on General, that choosing a page shows it and marks it
      current, and that a change survives moving to another page and back
- [x] 3.3 Add Reset to defaults to the footer, writing the active page's slice of `settingsDefaults()` through the
      draft's `update`; verify tests assert that resetting Export restores the four starting qualities and leaves a
      changed language alone, that Cancel after a reset restores everything, and that a reset Enhancements page saved
      clears the model choices and the exclusions
- [x] 3.4 Rebuild `rows.tsx` as `SettingsCard` and `SettingsRow` (label over description, control on the right,
      rules between rows) with the select and button controls at 180px, keeping the fill pins; remove the slider,
      radio and anchor machinery it no longer needs; verify `rows.test.tsx` asserts a row renders its label,
      description and control, and that the select and button keep their pinned fills

## 4. The four pages

- [x] 4.1 General: Language and Analytics in one card, the background as two live tiles (real `Particles` at the
      design's sample parameters, and dots from `--color-surface-dot` at a 16px pitch) with radios, Logs in its own
      card; verify tests assert both tiles render, that a click anywhere on a tile chooses it, that Show logs still
      reveals and a failed reveal still toasts, and that the chosen background is not applied before Save
- [x] 4.2 Performance: one radio card per offered processor with its sentence, Auto first with the Recommended badge
      and the chosen card outlined; verify tests assert a GPU report offers Auto, the provider and CPU, a CPU-only
      report offers two, every card's sentence renders whichever is chosen, and a stored provider this machine lacks
      reads as Auto
- [x] 4.3 Enhancements: the table (header, then icon and name, model select and Autopilot switch per
      `ENHANCEMENTS` entry); verify tests assert seven rows with detection absent, that choosing a model writes only
      that family, that a switch toggles only its own family in `autopilotExcluded`, that a never-chosen draft shows
      all seven switches on, and that each switch is addressable by its family's name
- [x] 4.4 Export: one card of four rows, each a filled slider and a typed box that holds its own text while focused
      and commits on blur or Enter (clamping out-of-range values, reverting empty or non-numeric ones); verify tests
      assert the four starting values, that typing 42 and pressing Enter moves the slider to 42, that 250 commits as
      100, that clearing and leaving reverts, that a drag updates the box, and that no starting-value mark is drawn

## 5. Verification

- [x] 5.1 Run the frontend `lint` (scoped to touched files), `typecheck` and `test` scripts; verify all three pass
- [x] 5.2 Launch the application on macOS and compare each page against screens 16, 16b, 16c and 16d - card and nav
      metrics, page heads, tiles, radio cards, the table and the export rows - in English and in German; verify
      Reset, Cancel and Save behave as specified and no page clips its content
