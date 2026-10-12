## MODIFIED Requirements

### Requirement: A setup that fails leaves the dialog in place with what it had

Where initialization fails, the setup dialog SHALL state the failure, rather than standing with its progress copy
over an application that has stopped. It is the same dialog in a different state, not a second dialog: the card,
the overall progress, the component list and the fact that none of it can be dismissed are the same in both, and
what changes is what is said about the attempt.

The failure dialog SHALL state that setting up could not be finished, and SHALL say that the application needs
these components before it can run. The failure SHALL be written to the application's log.

The application SHALL NOT be left usable behind either dialog, because nothing in it can run without the
components that did not arrive, and Quit SHALL remain the way out of both.

#### Scenario: A component fails to install

- **WHEN** initialization fails part way through
- **THEN** the setup dialog states that setting up could not be finished, in the place and at the size it already
  occupied
- **AND** the failure is recorded in the application's log
- **AND** Quit still exits the application

#### Scenario: The application behind the dialog

- **WHEN** initialization has failed and the failure dialog is shown
- **THEN** the application behind it cannot be used

## ADDED Requirements

### Requirement: The failure dialog keeps every component's outcome and points at the one that stopped

The failure dialog SHALL list the same components the setup dialog listed, in the same order and of the same
length, each still carrying its name and its published size.

Each row SHALL state that component's outcome: that it was installed, that it was never reached, or that it is
the one that stopped. The component that stopped is the one that was working when the failure arrived; there
SHALL be at most one, and it SHALL be marked as the failure rather than as work still in progress.

A component that was installed, and one that was never reached, SHALL be named, worded, iconed and coloured
exactly as the setup dialog names, words, icons and colours them - these are the same facts about the same
component, and only the row that stopped is the failure dialog's own state. A row that renamed itself or changed
its icon because the dialog around it changed would read as a state the component was never in.

No row SHALL show a progress track: a track is a measure of work under way, and no work is under way.

The **overall** progress SHALL be kept. How much of what this machine needs is on disk has the same answer it
had a moment before the failure, and with the failing component's own track gone it is the only thing still
saying how far the attempt got. It SHALL be stated once and not twice - so the header does not repeat the count
the bar carries.

The dialog SHALL be the size in its failed state that it was in its progress state. What a user is reading when
a failure arrives is the component list, and a list that moves as the dialog changes state is a list they have
to find again.

#### Scenario: A failure part way through the third of three components

- **WHEN** two components installed and the third failed while it was transferring
- **THEN** three rows are shown, the first two stating that they are installed and the third stating that it
  stopped
- **AND** no row shows a progress track, though the overall one is still shown
- **AND** the rows are where they were, because the dialog is the size it already was

#### Scenario: A component that was never reached

- **WHEN** a failure arrives while a component earlier in the order is working
- **THEN** the components after it state that they were never reached, in the setup dialog's own words and icon
  for a component it has not started

#### Scenario: A failure that belongs to no component

- **WHEN** initialization fails when no component was transferring - because every component was already
  installed, or because the failure was raised before any of them was reached
- **THEN** no row is marked as having stopped, and every row states the outcome it last reported

### Requirement: The reason for the failure is on screen and can be copied

The failure dialog SHALL show the reason the initialization gave, as its own text rather than as a summary or a
fixed sentence - so that two different failures do not read identically.

It SHALL be shown on **one line**, with what does not fit ellipsised rather than wrapped, so that the row
carrying it is the height the same row is while a component is transferring. A failure must not resize the list
it is reported in.

Clicking that text SHALL copy the **whole** reason - not the part of it that was drawn - and the dialog SHALL
confirm that it was copied, for long enough to read and no longer. This is what makes the truncation
affordable: what a user puts in a bug report is the sentence the library composed, URL and all. The text on
screen SHALL also be selectable, which the application prevents everywhere else.

Where a component is marked as having stopped, the reason SHALL be shown against that component, so which
failure belongs to which component needs no working out.

#### Scenario: A transfer that was reset

- **WHEN** a component's transfer fails because the connection was reset
- **THEN** the dialog shows the reason the initialization gave, against that component's row, on one line

#### Scenario: A reason longer than the row

- **WHEN** the reason does not fit on one line
- **THEN** it is ellipsised rather than wrapped, and the row is the height it would be with a progress track

#### Scenario: Copying the reason

- **WHEN** the user clicks the reason
- **THEN** the whole reason is on the clipboard, including the part that was not drawn
- **AND** the dialog says it was copied, and stops saying so shortly afterwards

### Requirement: The known cause is named only where it is the cause

The failure dialog SHALL state that these files are hosted on a service that throttles downloads and that
retrying in a few minutes usually works - but only where the failure was one of getting the files: a transfer
that did not complete, or files that arrived but did not match what was published.

It SHALL NOT state it for a failure of another kind. Advice to wait a few minutes and try again is false about a
runtime that will not load or a platform nothing is published for, and a dialog that gives it anyway is one a
user learns to stop reading.

#### Scenario: A download that failed

- **WHEN** initialization failed while transferring a component
- **THEN** the dialog states that the files are throttled by their host and that retrying in a few minutes
  usually works

#### Scenario: A runtime that would not load

- **WHEN** initialization failed because the runtime on disk could not be loaded
- **THEN** the dialog does not state the throttling explanation

### Requirement: Try again is offered where a second attempt could succeed

The failure dialog SHALL offer Try again, which SHALL begin a fresh initialization: the same one the application
starts itself with, not a narrower repair of the component that stopped.

While that attempt runs, the application SHALL show what it shows during any initialization - the setup dialog,
with the components and their progress - rather than a failure dialog with a spinner in it. A retry that
succeeds SHALL leave the application usable with no dialog; a retry that fails SHALL state the failure it
reached, which is not necessarily the one before it.

Try again SHALL NOT be offered where a second attempt cannot succeed: where nothing is published for this
machine, where the application was asked to run under a name it cannot use, or where a component has already
failed in a way this process cannot retry. Those leave Quit as the only action, because a button whose every
press reproduces the same error in an instant teaches a user that the application is broken rather than that the
situation is.

#### Scenario: Retrying a failed transfer

- **WHEN** the user chooses Try again after a transfer failed
- **THEN** a fresh initialization begins and the setup dialog is shown while it runs

#### Scenario: A retry that succeeds

- **WHEN** a retry completes successfully
- **THEN** no dialog is shown and the application is usable

#### Scenario: A retry that fails differently

- **WHEN** a retry fails for a different reason than the first attempt
- **THEN** the failure dialog states the reason the retry gave

#### Scenario: A failure a retry cannot fix

- **WHEN** initialization failed because no component is published for this operating system and architecture
- **THEN** Try again is not offered, and Quit is the only action

### Requirement: The failure dialog cannot be dismissed

The failure dialog SHALL NOT be dismissible: clicking outside it, pressing Escape and any close control SHALL
leave it in place, for the same reason the setup dialog cannot be dismissed - the application behind it cannot
be used, so a dismissed dialog would leave a user in front of an application that looks ready and does nothing.

#### Scenario: Trying to dismiss it

- **WHEN** the user clicks outside the failure dialog or presses Escape
- **THEN** the dialog stays where it is
