## ADDED Requirements

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

### Requirement: The processor choice offers what this machine turned out to support

The settings surface SHALL offer, as the processor the models run on, the providers this machine reported it offers,
plus an automatic choice that leaves the decision to the application.

The list SHALL be built from that report rather than from a list of providers the surface keeps, so a machine with no
supported graphics adapter is offered the automatic choice and its processor only, and a machine that gains one is
offered it without the surface being taught the name.

Each choice SHALL be accompanied by what picking it means for the user, and the automatic choice SHALL be the one the
application falls back to when a remembered provider is not among those offered.

Choosing a processor SHALL record the choice and nothing more. Nothing in the application runs a model yet, so there
is no session to rebuild and no memory to release; the slice that runs one reads what the user chose.

#### Scenario: A machine offering a GPU provider

- **WHEN** setup reported that this machine offers a GPU provider
- **THEN** the processor choice lists the automatic choice, that provider and the CPU

#### Scenario: A machine offering nothing but the CPU

- **WHEN** setup reported that this machine offers no provider beyond the CPU
- **THEN** the processor choice lists the automatic choice and the CPU, and nothing else

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

Each format SHALL carry its own starting value, and that value SHALL be shown alongside the control so the user can
see what the format was set to before they moved it. The scales are not comparable between encoders - the same number
is a different picture in each - so one value shared across formats, or one starting point applied to all of them,
would be a setting that means something different in every row it appears in.

A quality SHALL be bounded to what the encoders accept, and a remembered value outside those bounds or absent
altogether SHALL be replaced with that format's starting value. A quality of zero is not a harmless bad setting: it is
handed to the encoder as a genuine request.

#### Scenario: The formats and their starting values

- **WHEN** the user opens the export qualities without having changed any
- **THEN** AVIF reads 60, HEIC reads 60, JPEG reads 90 and WEBP reads 75

#### Scenario: One format's quality does not move another's

- **WHEN** the user changes one format's quality and saves
- **THEN** that format carries the new value and every other format carries the value it had

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
remember it.

Turning the choice off SHALL itself not be reported anywhere. An opt-out that is transmitted is not an opt-out.

The application SHALL send nothing on either setting until it has something to send; the choice is recorded so that
whatever eventually reports has the user's answer from before it existed, rather than assuming one.

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off and saves
- **THEN** the choice is remembered across restarts
- **AND** nothing is sent as a result of turning it off

### Requirement: The settings surface's sections can be navigated directly

The settings surface SHALL list its sections and the settings within them, and choosing one SHALL bring that setting
into view.

Every setting the surface holds SHALL appear in that list, and the list SHALL NOT name a setting the surface does not
show. A list entry that leads nowhere is the failure mode: it is indistinguishable from a setting the application
forgot to draw.

#### Scenario: Jumping to a setting

- **WHEN** the user chooses a setting from the navigation list
- **THEN** that setting is brought into view on the surface

#### Scenario: Every setting is reachable from the list

- **WHEN** the user opens the settings surface
- **THEN** every setting it shows is named in the navigation list, and every entry in the list leads to one
