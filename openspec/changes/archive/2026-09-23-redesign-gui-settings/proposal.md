## Why

The Settings dialog was built against screens 16 and 16b as one scrolling list of sixteen rows under an index of
every setting. The design has since been redrawn in Claude Design as screens 16, 16b, 16c and 16d: four flat pages,
each opening with a title and a line on what it covers, with rows grouped into cards and each page's controls chosen
for what they hold - radio cards for the processor, live tiles for the background, one table for the enhancements. The
redraw also adds two things the dialog cannot do today: telling Autopilot which enhancements it may suggest, and
putting a page back to its defaults.

Now because the library side of the first of those has just landed: `autopilot-family-filter` lets an analysis be
narrowed to a set of families, and nothing yet records which set a user wants.

## What Changes

- **Four pages instead of one scrolling list.** The nav becomes four entries with icons - General, Performance,
  Enhancements, Export - and choosing one shows that page alone. The index of sixteen settings and the scroll it drove
  are removed. **BREAKING** for the `gui-settings` requirement that the navigation lists every setting and brings it
  into view.
- **Each page opens with its title and a one-line description**, replacing the grey section band.
- **General**: Language and Analytics in one card, the preview background as two live tiles (the real particle field
  and the dot grid) with a radio under each, and Logs in a card of its own. Every row's copy is shortened to the
  design's.
- **Performance**: the processor becomes radio cards, one per option this machine offers, each with a sentence on what
  picking it means; the automatic choice carries a **Recommended** tag. The options are still built from the provider
  report.
- **Enhancements**: the seven default-model rows become one table - enhancement, default model, and a new
  **Autopilot** switch per enhancement.
- **A new remembered preference: which enhancements Autopilot may suggest.** All seven are on for a user who has never
  chosen, so Autopilot behaves as it does today until someone opts a family out. Nothing in the GUI runs an analysis
  yet; the slice that does narrows it to this set.
- **Export**: the four qualities share one card, each row a filled slider and a box the value can be typed into. A
  typed value is clamped to the accepted bounds when the box is left or Enter is pressed, and an emptied box goes back
  to the value it held. The tick marking each format's starting value is removed, as the design draws none.
- **Reset to defaults**, in the footer, puts the settings on the page being viewed back to their defaults - in the
  draft, like every other change, so nothing is applied until Save and Cancel still undoes it.
- **New copy in all thirteen locales**: page names and descriptions, the shortened row copy, the per-processor
  sentences, the Recommended tag, the table headings and Reset to defaults. Keys nothing reads any more are removed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-settings`: the navigation requirement is replaced by one about pages; the processor requirement gains the
  per-option explanation and the recommended automatic choice as a visible rule; the export-quality requirement drops
  the starting-value mark and gains typed entry with its clamping; two requirements are added - the Autopilot
  preference per enhancement, and resetting a page to its defaults.

## Impact

**Crates: `gui` only**, and within it the frontend. No Tauri command is added or widened, no capability is granted,
and no `Cargo.toml` is touched. `opai` and `cli` are untouched.

The one piece of logic that looks like it might belong in `opai` is the Autopilot preference. It does not: it is a
choice a user makes about this application's own Autopilot, stored beside the other settings-dialog preferences, and
the library already takes the set as an argument on each analysis - which is where a front end hands its preference
over. `cli` would take the same set from a flag, not from this store.

**rust-sak: nothing used and nothing missing.** There is no Rust and no infrastructure in this change.

**New npm dependencies: none.** One generated component, `input`, through `shadcn add input`, with the hand-edits
every `shadcn add` in this crate needs.

**Task estimate: thirteen**, so one change rather than a series.

**Parity: three deliberate divergences from the reference's behaviour.**

- **Autopilot can be narrowed per enhancement.** The reference's Autopilot is one switch and always checks every
  family. The narrowing is new in `opai`, and this is where a user reaches it; with every family on - the default - the
  behaviour is the reference's.
- **Reset to defaults** has no counterpart in the reference. It only changes the draft, so it adds a way back without
  adding a way to lose anything.
- **No starting-value mark under the export sliders**, where the reference labels one. The design removed it; what the
  mark offered - getting back to where a format started - is now Reset to defaults on the Export page.
