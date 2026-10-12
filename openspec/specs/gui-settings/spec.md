# gui-settings Specification

## Purpose

What the graphical application's settings surface can know and do: which execution providers the machine in front of
it turned out to offer, where its own log file is and how a user is shown it, and which models each enhancement can
be set to default to. These are the facts the settings dialog is drawn from, and they are answered by the application
whether or not a dialog is open.

## Requirements

### Requirement: A completed setup reports what this machine can be asked to run on

Once the application has finished setting itself up, it SHALL report which execution providers this machine offers.
The report SHALL name every provider the application can be asked for, including the one that is always available, so
that a list of choices can be built from it without anything outside knowing which providers exist.

The report SHALL describe what was actually installed on this machine, not merely what hardware was detected. A
machine whose graphics adapter is supported but for which no library was published SHALL NOT be reported as offering
that adapter's provider - otherwise the application would offer a choice that can only fall back.

A request that arrives after the application is already set up SHALL be answered with the same report without
installing, verifying or transferring anything again. Setting up is what decides the answer, and asking again is not
a reason to do it twice.

#### Scenario: A machine with an NVIDIA RTX adapter and its libraries installed

- **WHEN** setup finishes on a machine where the CUDA and TensorRT libraries were installed
- **THEN** the report names CUDA and TensorRT among the providers this machine offers
- **AND** it names the CPU provider as well

#### Scenario: Hardware is present but nothing was published for the platform

- **WHEN** setup finishes on a machine with an NVIDIA adapter on a platform no NVIDIA library is published for
- **THEN** the report does not name that adapter's providers
- **AND** the application offers only the providers whose libraries are on this machine

#### Scenario: The report is asked for again

- **WHEN** the application is asked to set up a second time in the same session, after the first succeeded
- **THEN** it answers with the same report
- **AND** nothing is downloaded, expanded, verified or loaded again

### Requirement: The application can say where its log file is without creating anything

The application SHALL be able to report the location of its own log file. Asking SHALL create no directory, open no
file, install no log sink and write no record, so that the question can be asked at any time - including by a screen
that is merely being drawn - without changing anything on disk.

The location SHALL be the one this application actually logs to, so a user directed there finds the file the
application has been writing.

#### Scenario: Asking where the log is

- **WHEN** the application is asked where its log file is
- **THEN** it answers with the path of the file it logs to

#### Scenario: Asking is free of side effects

- **WHEN** the application is asked where its log file is on a machine where it has never run before
- **THEN** it answers with a path
- **AND** no directory and no file has been created at that path

#### Scenario: The location cannot be resolved

- **WHEN** the platform offers no configuration directory to resolve the location from
- **THEN** the application reports that the location could not be determined
- **AND** nothing is created

### Requirement: The log file can be shown to the user in the platform's file manager

The application SHALL be able to open the running platform's own file manager with its own log file selected, so a
user can reach that file without being asked to type or copy its path.

Which file is shown SHALL be decided where the location is resolved rather than supplied by the surface asking for
it, so that the file revealed and the location reported cannot be made to disagree. Revealing an arbitrary path is
not something the application offers.

This SHALL work on each of the application's three shipping platforms, using that platform's own file manager. A
failure to open it SHALL be reported to the caller rather than passing silently, because the caller is the path a
user is being given for diagnosing something else.

#### Scenario: Revealing the log file

- **WHEN** the application is asked to show its log file
- **THEN** the platform's file manager opens with that file selected
- **AND** it is the same file the application reports its log's location to be

#### Scenario: The file manager could not be opened

- **WHEN** the application is asked to show its log file and the platform's file manager cannot be opened
- **THEN** the failure is reported to the caller

### Requirement: The models each enhancement offers are published to the application's own interface

The application SHALL publish, to its own interface, the catalogue of what the library can be asked to run: every
enhancement family, the models within it, each model's display name and the precisions it is published in, and the
parameters that model takes with the bounds a control may offer.

It SHALL be the library's catalogue unchanged. The application's interface SHALL NOT carry a list of model names,
labels or bounds of its own - that duplication is what lets a control offer a model that no longer exists or a value
the library would refuse.

Every family SHALL be published, including families with no adjustable parameter, so that "this enhancement has
nothing to configure" is something the interface is told rather than something it infers from an absence.

#### Scenario: Reading what a family offers

- **WHEN** the interface asks for the catalogue
- **THEN** it receives every enhancement family with the models each one offers, their display names and their
  published precisions

#### Scenario: A family with nothing to adjust

- **WHEN** the interface asks for the catalogue
- **THEN** a family whose models take no adjustable parameter is present with an empty parameter list, rather than
  being left out

### Requirement: The application offers a settings surface that can be opened and dismissed without changing anything

The application SHALL offer a way to reach its settings from the window's own chrome, and that surface SHALL be
dismissible by the user at any time.

Opening it SHALL change nothing on its own. A surface that had to be saved to be closed would make reading a setting
an act of changing it.

The surface SHALL NOT be dismissed by a click outside it. Everything on it is a draft until the user says otherwise,
and a stray click is not an answer to "keep this or not" - which is the one interaction the reference implementation
also refuses, while accepting both the close control and the keyboard's dismiss key.

#### Scenario: Opening and closing the settings surface

- **WHEN** the user asks for the application's settings
- **THEN** the settings surface is shown
- **AND** closing it leaves every preference exactly as it was

#### Scenario: A click outside does not dismiss it

- **WHEN** the settings surface is open and the user clicks outside it
- **THEN** it stays open and nothing is discarded

### Requirement: A setting is a draft until it is saved, and cancelling restores everything

While the settings surface is open, every preference the user changes SHALL be held as a draft. Saving SHALL make
every draft the application's actual preference. Cancelling or dismissing SHALL restore every preference to what it
was when the surface was opened, with no exception for any individual setting.

Nothing outside the surface SHALL observe a draft. A preference that is acted on the moment a control is touched
cannot be undone by cancelling, so anything the application does in response to a preference SHALL happen on save
rather than on the change - which is why the chosen language takes effect only once the user saves.

A draft SHALL NOT be recorded where preferences survive a restart. Saving is what records one. Quitting, crashing or
losing power while the surface is open is the one exit that neither button is on: a draft written down as the control
moved would outlive the session that made it and be read at the next launch as a preference the user chose, which is
a cancellation the user can never perform.

Restoring SHALL cover every preference the surface holds. A setting that is added to the surface and left out of the
restore is one that cancelling silently keeps, which is indistinguishable to the user from the application ignoring
the button.

#### Scenario: Cancelling several changes at once

- **WHEN** the user changes the language, the processor, a default model and an export quality, then cancels
- **THEN** all four are what they were before the surface was opened
- **AND** the application is still in the language it was in

#### Scenario: Saving applies the language

- **WHEN** the user picks a different language and saves
- **THEN** the application's text is shown in the chosen language

#### Scenario: A language picked but not saved is never applied

- **WHEN** the user picks a different language and then cancels
- **THEN** the application's text is unchanged throughout, before and after

#### Scenario: The application stops while a draft is open

- **WHEN** the user changes a setting and the application stops before they save or cancel
- **THEN** the next launch reads that preference as it was before the surface was opened

### Requirement: The preferences the settings surface holds survive a restart

Every preference the settings surface holds SHALL be remembered across restarts of the application, so a choice is
made once rather than at every launch.

A remembered preference that the running machine or the current version of the application can no longer honour SHALL
be replaced with a supported one rather than kept. Preferences outlive the machine they were chosen on - a graphics
driver is removed, a model stops being published, a language catalogue is dropped - and a preference that names
something absent is not a setting that merely looks wrong: it is one that makes the operation behind it fail or shows
the user a control with no answer in it.

#### Scenario: A choice is still there after a restart

- **WHEN** the user saves a language, a processor, a default model and an export quality, and restarts the application
- **THEN** all four are the values that were saved

#### Scenario: A remembered value the application no longer offers

- **WHEN** the application starts and a remembered preference names something it no longer offers
- **THEN** that preference reads as a supported value
- **AND** every other remembered preference is left as it was

### Requirement: The user can choose the application's language from the languages it ships

The settings surface SHALL offer every language the application ships a catalogue for, and no others.

Each language SHALL be named in that language itself. Someone who cannot read the language the application is
currently showing must still recognise their own, which a list translated into the active language would prevent.

Until the user has chosen, the application SHALL start in the language its environment asks for where that is one it
ships, and in English otherwise. The environment's ordered list of preferred languages SHALL be considered rather than
its first entry alone, and a preference naming a region SHALL be matched on the language itself, so a user whose
region the application ships no separate catalogue for still gets their language.

#### Scenario: A first launch in a language the application ships

- **WHEN** the application is launched for the first time on a machine asking for a language it ships
- **THEN** the application starts in that language

#### Scenario: A first launch in a language the application does not ship

- **WHEN** the application is launched for the first time on a machine asking only for languages it does not ship
- **THEN** the application starts in English

#### Scenario: The list of languages

- **WHEN** the user opens the language choice
- **THEN** each language is named in itself, whatever language the application is currently showing

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

### Requirement: Each enhancement can be given the model it defaults to

The settings surface SHALL let the user choose, for each enhancement the application offers, which of that
enhancement's models is used when an enhancement is added without the user picking one.

The models on offer, their names and the quality levels each is published at SHALL come from the catalogue the
application publishes, so no enhancement's models are restated by the surface and a model added to the library appears
here without the surface being edited.

Each model SHALL be offered at every quality level it is published at, because the level is part of the choice: it
decides what an enhancement is built at when it is added. Where a model is published at fewer or different levels than
its siblings, the levels offered for it SHALL be its own.

Where the user has expressed no preference for an enhancement, the model offered first for that enhancement SHALL be
the one it defaults to, at the first quality level that model offers.

#### Scenario: Choosing a default model

- **WHEN** the user picks a model and a quality level for an enhancement and saves
- **THEN** that enhancement's default is the chosen model at the chosen level

#### Scenario: An enhancement never chosen for

- **WHEN** the user has expressed no preference for an enhancement
- **THEN** its default reads as the first model the catalogue offers for it, at that model's first quality level

#### Scenario: A model published at its own quality levels

- **WHEN** the user opens the choice for an enhancement one of whose models is published at different levels from its
  siblings
- **THEN** that model is offered at its own levels and the others at theirs

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

### Requirement: The settings surface can show the user their log file, and says so when it cannot

The settings surface SHALL offer the user a way to be shown this application's log file in the platform's own file
manager.

A failure to show it SHALL be reported to the user in the language the application is running in. This is the control
whose whole purpose is to help a user attach a log to a report about something else; a button that quietly does
nothing leaves them with neither the file nor a reason, which is worse than the failure itself.

#### Scenario: Showing the log file

- **WHEN** the user asks to be shown the log file
- **THEN** the platform's file manager opens with that file selected

#### Scenario: The log file could not be shown

- **WHEN** the user asks to be shown the log file and it cannot be shown
- **THEN** the user is told it could not be opened and where to find it

### Requirement: The user can decline anonymous usage analytics

The settings surface SHALL carry a choice about whether the application may send anonymous usage analytics, and SHALL
remember it. It SHALL default to on.

The application SHALL honour the choice. It governs everything the application sends: the backend's records, spans
and metrics, and the window's usage events, failures and session. While it is off, the application SHALL send
nothing to any collector:
- **A launch that begins with it off** SHALL send nothing, from its first record to its last, from the backend or the
  window.
- **Turning it off and saving** SHALL stop sending at once, from both. Nothing emitted after the save SHALL be sent,
  although what was already sent stays sent.
- **Turning it on and saving** SHALL take effect from the next launch, for both. The session in which it was turned on
  SHALL NOT resume sending from either.

Turning the choice off SHALL itself not be reported anywhere. An opt-out that is transmitted is not an opt-out.

The choice the user sees in the settings surface SHALL be the one honoured. If the application finds that its record
of the choice disagrees with what the settings surface shows, the settings surface's value SHALL win, and turning it
off SHALL still stop sending at once.

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off and saves
- **THEN** the choice is remembered across restarts
- **AND** nothing is sent as a result of turning it off
- **AND** nothing the application or its window does for the rest of the session is sent

#### Scenario: A launch after declining

- **WHEN** the user has turned analytics off and then starts the application
- **THEN** no collector receives anything from that session, from the backend or the window

#### Scenario: Turning analytics back on

- **WHEN** the user who had turned analytics off turns it on and saves
- **THEN** nothing further is sent during that session, from the backend or the window
- **AND** the next launch sends as a launch with analytics on does

#### Scenario: A first launch

- **WHEN** the application is started for the first time, in a build that names a collector
- **THEN** it sends, because the choice defaults to on

#### Scenario: The window reloads after declining

- **WHEN** the user turns analytics off and the window is then reloaded in the same session
- **THEN** the reloaded window sends nothing

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
Switching every enhancement off SHALL leave Autopilot suggesting nothing, and SHALL NOT be an error. The choice in
force is the saved one at the moment an analysis is asked for. Saving a different choice SHALL NOT stop or change an
analysis already under way, and SHALL NOT analyse again a photograph that has already been analysed.

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

#### Scenario: The choice governs the next analysis

- **WHEN** the user switches colorization off for Autopilot and saves, then opens a grayscale photograph with
  Autopilot on
- **THEN** the photograph is analysed and no colorization is added to it

#### Scenario: Every enhancement switched off

- **WHEN** the user switches every enhancement off for Autopilot and saves, then opens a photograph with Autopilot on
- **THEN** nothing is added to it and nothing is reported

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

### Requirement: The answer to the TensorRT question sets the processor, and is remembered apart from the preferences

Answering the TensorRT question (see `gui-setup`) SHALL set the processor preference: **Yes** to the automatic choice,
**No** to CUDA. The answer SHALL be applied and recorded at once, exactly as a processor saved from the settings
surface is - there is no draft to save or cancel, because the question is not the settings surface.

That the question was answered SHALL be remembered across restarts, apart from the preferences the settings surface
holds. It is not a preference: the settings surface SHALL NOT show it, and resetting a page to its defaults SHALL NOT
clear it, since doing so would ask a user who has already answered to answer again.

An answer recorded by the application this one replaces, under the same stored preferences, SHALL be read as the
question having been answered, so a user upgrading is not asked a second time.

#### Scenario: Answering Yes

- **WHEN** the user answers Yes
- **THEN** the processor preference is the automatic choice
- **AND** the Performance page shows the automatic choice selected

#### Scenario: Answering No

- **WHEN** the user answers No
- **THEN** the processor preference is CUDA
- **AND** the Performance page shows CUDA selected

#### Scenario: The answer survives a restart

- **WHEN** the user answers the question and restarts the application
- **THEN** the processor preference is the one the answer set
- **AND** the question is not asked again

#### Scenario: Resetting the Performance page after answering

- **WHEN** the user answered No, then resets the Performance page and saves
- **THEN** the processor preference is the automatic choice
- **AND** the question is not asked on the next launch

#### Scenario: A user upgrading from the application this one replaces

- **WHEN** the application starts with stored preferences in which that application recorded the question as answered
- **THEN** the question is not asked
