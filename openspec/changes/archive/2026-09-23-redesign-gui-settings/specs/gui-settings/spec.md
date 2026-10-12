## ADDED Requirements

### Requirement: The settings surface is divided into pages, one shown at a time

The settings surface SHALL divide its preferences into four pages - General, Performance, Enhancements and Export -
and SHALL show one page at a time. A list of the four pages SHALL always be visible beside it, and choosing a page
from that list SHALL show that page in place of the one before it.

Each page SHALL open with its own name and one line saying what the page covers, so a user who arrives on a page can
tell what it is for without reading its controls.

Every preference the surface holds SHALL be on exactly one page. A preference on no page is one the user cannot
reach; a preference on two is one that can be changed in one place and read back in another.

Changing pages SHALL NOT discard, apply or reset anything. Every preference is a draft until Save wherever it was
changed, so a change made on one page SHALL still be there after moving to another page and back, and SHALL be
applied or discarded with the rest.

The surface SHALL open on the General page.

#### Scenario: Opening the settings surface

- **WHEN** the user opens the settings surface
- **THEN** the General page is shown, with its name and its one-line description
- **AND** the list of the four pages is shown beside it, with General marked as the current page

#### Scenario: Moving to another page

- **WHEN** the user chooses Export from the list of pages
- **THEN** the Export page is shown in place of the page before it
- **AND** Export is marked as the current page

#### Scenario: A change survives moving between pages

- **WHEN** the user changes the language, moves to the Export page, then back to General
- **THEN** the language still reads as the user changed it
- **AND** nothing has been applied

#### Scenario: Every preference has a page

- **WHEN** the pages are compared with the preferences the surface holds
- **THEN** every preference is on exactly one page

### Requirement: The user can choose which enhancements Autopilot may suggest

The settings surface SHALL let the user choose, for each enhancement the application offers, whether Autopilot may
suggest it. The choice SHALL be shown beside that enhancement's default model, so each enhancement is listed once with
everything the surface holds about it.

An enhancement Autopilot may not suggest SHALL still be available to add by hand. The choice governs what Autopilot
proposes, not what the application can do.

Where the user has expressed no preference, Autopilot SHALL be allowed to suggest every enhancement, so that Autopilot
behaves as it did before the choice existed until the user opts an enhancement out. An enhancement the application
offers that the user has never been asked about - one added in a later version - SHALL likewise start allowed.

The choice SHALL be remembered across restarts and be a draft until saved, like every other preference the surface
holds. A remembered choice that cannot be read SHALL be treated as allowing every enhancement, by the rule every other
remembered preference already follows.

Whenever the application analyses an image for Autopilot, it SHALL ask for only the enhancements the user allows.
Switching every enhancement off SHALL leave Autopilot suggesting nothing, and SHALL NOT be an error. The application
does not run an Autopilot analysis yet; the choice is recorded so that whatever first runs one has the user's answer
from before it existed, rather than assuming one.

#### Scenario: A user who has never chosen

- **WHEN** the user opens the Enhancements page without having changed any Autopilot choice
- **THEN** every enhancement is shown as one Autopilot may suggest

#### Scenario: Opting an enhancement out

- **WHEN** the user switches Autopilot off for colorization and saves
- **THEN** colorization is recorded as an enhancement Autopilot may not suggest
- **AND** colorization can still be added by hand
- **AND** the choice is still in effect after a restart

#### Scenario: A choice made but not saved

- **WHEN** the user switches Autopilot off for an enhancement and cancels
- **THEN** that enhancement is still recorded as one Autopilot may suggest

#### Scenario: A remembered choice that cannot be read

- **WHEN** the application starts and the remembered Autopilot choice is not something it can read
- **THEN** every enhancement reads as one Autopilot may suggest
- **AND** every other remembered preference is left as it was

### Requirement: A page's settings can be reset to their defaults

The settings surface SHALL offer a way to put the settings on the page being viewed back to their defaults. Only that
page's settings SHALL change; the settings on the other pages SHALL be left as they are in the draft.

A reset SHALL change the draft and nothing else. Nothing SHALL be applied until the user saves, and cancelling after a
reset SHALL leave every preference as it was when the surface was opened.

A setting's default SHALL be the value it reads as for a user who has never chosen: the language the application
would start in on a first launch, the particle field, analytics on, the automatic processor, each enhancement's first
catalogue model at its first quality level, every enhancement allowed for Autopilot, and each export format's
starting quality.

#### Scenario: Resetting the Export page

- **WHEN** the user changes the JPEG quality and the language, then resets the Export page
- **THEN** every export quality reads its starting value
- **AND** the language still reads as the user changed it

#### Scenario: A reset is a draft

- **WHEN** the user resets a page and then cancels
- **THEN** every preference is what it was before the surface was opened

#### Scenario: A reset that is saved

- **WHEN** the user resets the Enhancements page and saves
- **THEN** every enhancement defaults to its first catalogue model at its first quality level
- **AND** Autopilot may suggest every enhancement

## MODIFIED Requirements

### Requirement: The user can choose what is drawn behind the image

The settings surface SHALL carry a choice of what the preview canvas draws behind the image, offering exactly two
surfaces: a field of drifting particles and a grid of dots. The choice SHALL be presented as a set of named options
of which exactly one is chosen at a time, so that both surfaces and which of them is in effect are visible without
opening anything. Each option SHALL show a live sample of the surface it names, so the user sees what they are
choosing rather than only its name.

The two SHALL be exclusive rather than cumulative. Choosing one SHALL replace the other entirely; neither is ever
drawn over the other, and nothing else about the canvas changes with the choice.

An application that has never been told SHALL draw the particle field. This is a preference with no wrong answer -
both surfaces are decorative, and neither affects what the application can do - so it is started at the one the
design's own screens open on rather than left unset.

The choice SHALL be a draft until it is saved, like every other preference the surface holds. Nothing outside the
surface observes a draft, so the canvas behind the dialog SHALL keep the surface it had until the user saves, and
cancelling SHALL leave it unchanged throughout. This is the same rule the chosen language already follows and for
the same reason: a background applied as the control was touched could not be taken back by Cancel.

A remembered choice naming neither surface SHALL read as the particle field, by the rule every other remembered
preference already follows.

#### Scenario: Choosing the dotted surface

- **WHEN** the user chooses the dotted surface and saves
- **THEN** the preview canvas draws a grid of dots and no particles
- **AND** the choice is still in effect after a restart

#### Scenario: Both surfaces are shown as samples

- **WHEN** the user opens the General page
- **THEN** the particle field and the grid of dots are each drawn as a sample beside their names

#### Scenario: An application that has never been told

- **WHEN** the application is started for the first time and the user has chosen no background
- **THEN** the preview canvas draws the particle field
- **AND** the settings surface shows the particle field as the chosen option

#### Scenario: A background picked but not saved is never applied

- **WHEN** the user picks the other background and then cancels
- **THEN** the preview canvas draws the same surface throughout, before and after
- **AND** the settings surface shows the original choice again

#### Scenario: A remembered choice naming neither surface

- **WHEN** the application starts and the remembered background names neither surface
- **THEN** the preview canvas draws the particle field
- **AND** every other remembered preference is left as it was

### Requirement: The processor choice offers what this machine turned out to support

The settings surface SHALL offer, as the processor the models run on, the providers this machine reported it offers,
plus an automatic choice that leaves the decision to the application.

The list SHALL be built from that report rather than from a list of providers the surface keeps, so a machine with no
supported graphics adapter is offered the automatic choice and its processor only, and a machine that gains one is
offered it without the surface being taught the name.

Every choice SHALL be shown at once, each with what picking it means for the user, so the trade-off between them can
be read before choosing rather than only after. The automatic choice SHALL be marked as the recommended one, and SHALL
be the one the application falls back to when a remembered provider is not among those offered.

Choosing a processor SHALL record the choice and nothing more. Nothing in the application runs a model yet, so there
is no session to rebuild and no memory to release; the slice that runs one reads what the user chose.

#### Scenario: A machine offering a GPU provider

- **WHEN** setup reported that this machine offers a GPU provider
- **THEN** the processor choice lists the automatic choice, that provider and the CPU

#### Scenario: A machine offering nothing but the CPU

- **WHEN** setup reported that this machine offers no provider beyond the CPU
- **THEN** the processor choice lists the automatic choice and the CPU, and nothing else

#### Scenario: Every choice explains itself before it is chosen

- **WHEN** the user opens the Performance page
- **THEN** every processor offered is shown with its own explanation, whichever one is chosen
- **AND** the automatic choice is marked as recommended

#### Scenario: A remembered provider this machine no longer offers

- **WHEN** the application starts on a machine that no longer offers the remembered provider
- **THEN** the processor reads as the automatic choice

### Requirement: Each lossy export format keeps its own quality

The settings surface SHALL let the user set an encoding quality for each export format whose encoder takes one, and
SHALL keep one value per format rather than one shared across them.

Each format SHALL carry its own starting value. The scales are not comparable between encoders - the same number is a
different picture in each - so one value shared across formats, or one starting point applied to all of them, would be
a setting that means something different in every row it appears in.

Each quality SHALL be settable both by dragging and by typing the number, and the two SHALL always show the same
value. A typed value SHALL NOT take effect while it is being typed; when the user leaves the field or confirms it, a
value outside the accepted bounds SHALL be brought to the nearest bound, and a field left empty or holding something
that is not a number SHALL go back to the value it held before.

A quality SHALL be bounded to what the encoders accept, and a remembered value outside those bounds or absent
altogether SHALL be replaced with that format's starting value. A quality of zero is not a harmless bad setting: it is
handed to the encoder as a genuine request.

#### Scenario: The formats and their starting values

- **WHEN** the user opens the export qualities without having changed any
- **THEN** AVIF reads 60, HEIC reads 60, JPEG reads 90 and WEBP reads 75

#### Scenario: One format's quality does not move another's

- **WHEN** the user changes one format's quality and saves
- **THEN** that format carries the new value and every other format carries the value it had

#### Scenario: Typing a quality

- **WHEN** the user types 42 into JPEG's quality and confirms it
- **THEN** JPEG's quality reads 42, and its slider shows 42

#### Scenario: Typing a quality outside the bounds

- **WHEN** the user types 250 into a format's quality and leaves the field
- **THEN** that format's quality reads 100

#### Scenario: Leaving the typed quality empty

- **WHEN** the user clears a format's quality and leaves the field
- **THEN** that format's quality reads the value it held before it was cleared

#### Scenario: A remembered quality that cannot be honoured

- **WHEN** the application starts and a remembered quality is absent, zero or outside the accepted bounds
- **THEN** that format reads its starting value

## REMOVED Requirements

### Requirement: The settings surface's sections can be navigated directly

**Reason**: The surface no longer lists every setting in one scrolling pane under an index of them. It is divided
into four pages shown one at a time, so there is no position to bring a setting into and no per-setting entry to
choose.

**Migration**: Replaced by "The settings surface is divided into pages, one shown at a time", which keeps the
guarantee this requirement existed for - no preference that cannot be reached - as "every preference is on exactly
one page".
