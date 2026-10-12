# gui-setup Specification

## Purpose

What the graphical application does between being launched and being ready to use: the order its startup
steps run in, that it begins without being asked, what it tells the user about the components this machine
needs while they are being put in place, and the way out of a launch that has not finished.

## Requirements

### Requirement: The library search path is established before anything else the application does

The graphical application SHALL establish its library search path as the very first thing it does, ahead of
installing its log sink, ahead of any single-instance handling, and ahead of creating any window or any other
thread.

This is the one ordering in startup that cannot be recovered from later: on a platform whose loader reads its
search path once at process start, the step replaces the process image, and anything that ran before it ran
in a process that is about to be discarded - including a log sink, whose file handle the replacement would
inherit with nobody left holding the other end.

A search path that cannot be established SHALL NOT stop the application from starting. The failure SHALL be
reported once the log sink exists, and startup SHALL continue - the cost is the GPU execution providers, not
the launch.

#### Scenario: The order of the startup steps

- **WHEN** the graphical application is launched
- **THEN** the library search path is established before the log sink is installed, before a second launch
  can be detected and before any window exists

#### Scenario: The search path could not be established

- **WHEN** the library directories cannot be created or the search path cannot be established
- **THEN** the failure is reported in the application's log
- **AND** the application goes on starting

### Requirement: The application starts itself

The graphical application SHALL begin initializing as soon as its window is ready, without the user asking
for it. There SHALL be no control to press to start it and no prompt to accept first.

Initialization is what puts the ONNX Runtime and the GPU libraries on disk and starts the inference
environment, and nothing in the application can be used before it has finished, so making a user request it
would be asking them to confirm the only thing that can happen next.

#### Scenario: A first launch

- **WHEN** the application is launched on a machine where nothing is installed yet
- **THEN** it begins installing what the machine needs without the user doing anything

### Requirement: The application initializes once per session, however many times it is asked to

The application SHALL carry out at most one initialization at a time, and once one has succeeded SHALL NOT
carry out another for the life of the process: a later request SHALL answer from the first success without
transferring, expanding or verifying anything again.

A request that arrives while an initialization is running SHALL wait for that one rather than starting a
second. Two installs of the same archive writing the same file, and two inference environments racing to
start, are the outcomes this exists to prevent - and the requests genuinely overlap, because the window's
own start-up asks for one, a development build's double-mounted start-up asks again, and a retry asks a
third time.

A failed initialization SHALL remain retryable: only success is remembered.

#### Scenario: Two requests at once

- **WHEN** a second initialization is requested while the first is still running
- **THEN** no second install begins, and both requests are answered by the one initialization

#### Scenario: A request after the application is ready

- **WHEN** initialization is requested again after one has already succeeded
- **THEN** it succeeds immediately, and nothing is downloaded, expanded or verified

#### Scenario: A request after a failure

- **WHEN** initialization is requested again after one has failed
- **THEN** a fresh initialization is carried out

### Requirement: The setup dialog appears only when there is work to do

The application SHALL show its setup dialog when the first component actually begins transferring, and SHALL
NOT show it before that.

A launch on which every component is already installed SHALL show no dialog at all, however briefly - the
application simply becomes ready.

Opening on the first transfer rather than on the list of components is what keeps the dialog honest: by the
time any byte moves, every component ahead of that one in the order has already reported whether it had work
to do, so no row a user sees is drawn as waiting when it is in fact already installed.

#### Scenario: A first launch

- **WHEN** initialization begins transferring the first component
- **THEN** the setup dialog is shown over the application window

#### Scenario: A launch with everything already installed

- **WHEN** the application is launched on a machine where every component is installed and intact
- **THEN** no setup dialog is shown at any point
- **AND** the application becomes ready

### Requirement: The dialog shows every component this machine needs

While the setup dialog is shown, it SHALL list every component this machine's initialization accounts for -
including those that turned out to need no work - in the order they are handled, each carrying its name and
the published size of its files.

Each row SHALL state that component's own state: that it is already installed, that it is being transferred,
or that it has not been reached yet. A row being transferred SHALL carry its own progress. The list SHALL
have the same length throughout one launch, and SHALL NOT grow as components report.

A component that has finished SHALL state that it is installed and SHALL give up its progress track, whichever
phase it was in when it finished. Because a component comes to rest in the phase it finished in, a dialog that
read the reported phase alone left a finished row stating that it was still expanding for the rest of the
launch, with the next row transferring beneath it - two components working at once, which the library never
does.

A component SHALL make that change **at the moment the component after it begins**, so that one row becomes
installed and the next begins transferring together. The gap between a component's last report and its
successor's first is real work - the next component's record is checked and its connection opened before a
byte arrives to report - and a dialog that changes at each end of that gap separately shows a list in which
nothing is happening for the length of it. A component with nothing after it SHALL state that it is installed
as soon as it has finished, there being no hand-off to wait for and the dialog remaining while the runtime
loads.

This applies only while setting up is under way. Once an attempt has stopped, a component's outcome SHALL be
read from how far it actually got: a failure raised during a hand-off belongs to the component being started
or to no component, and SHALL NOT be attributed to the one that had just finished.

The rows SHALL be keyed by the component they name, so that a component reporting several times occupies one
row rather than appearing more than once.

#### Scenario: A first launch on a machine with an RTX card

- **WHEN** the setup dialog is shown on a machine that needs the runtime, CUDA, cuDNN and TensorRT
- **THEN** four rows are shown, in the order they will be installed, each with its name and its published
  size

#### Scenario: A component that was already installed

- **WHEN** a component reports that it was already in place
- **THEN** its row states that it is installed and shows no transfer in progress

#### Scenario: A component that has not been reached

- **WHEN** a component has reported nothing yet
- **THEN** its row states that it is waiting, and shows no progress track

#### Scenario: One component finishes and the next begins

- **WHEN** a component finishes expanding and the next one starts transferring
- **THEN** the finished row states that it is installed and has no track, and only the next row shows work in
  progress - so that no two rows ever describe work happening at the same time

#### Scenario: The gap between one component and the next

- **WHEN** a component has finished and the one after it has not yet reported anything
- **THEN** the finished row still states that it is finishing, and changes to installed on the same report
  that starts the next component - so the list never stands with nothing happening in it

#### Scenario: A failure during a hand-off

- **WHEN** an attempt fails after a component finished and before the next one reported
- **THEN** the component that finished states that it is installed rather than being marked as the one that
  stopped

#### Scenario: Repeated reports for one component

- **WHEN** a component reports its progress many times
- **THEN** it occupies exactly one row, whose progress is the most recent report

### Requirement: The dialog says how far along the whole setup is

The setup dialog SHALL show one overall measure of progress across every component, beside the number of
components finished out of the number the machine needs.

The overall measure SHALL reflect how much of the work remains rather than how many components remain, so
that a small component finishing does not advance it as much as a large one. Components of this set differ
in size by more than an order of magnitude, and a figure derived from the count alone would jump.

The denominator SHALL be every component this machine needs, including those already installed, so that it
does not change between two launches of the same application on the same machine.

#### Scenario: One of three components installed

- **WHEN** the machine needs three components, one is already installed and a larger one is part way through
- **THEN** the dialog states that one of three components is installed
- **AND** the overall measure is between the two, weighted by how large each component is rather than by how
  many there are

#### Scenario: The denominator counts what is already there

- **WHEN** two of the machine's three components were already installed before this launch
- **THEN** the dialog still reports three components in total

### Requirement: The dialog explains why setting up can be slow

The setup dialog SHALL state that these files are hosted on a service that throttles downloads when they are
fetched often, and that this can make the transfer slower than the user's connection allows.

Without it, a throttled transfer is indistinguishable from an application that has hung, which is the reading
this sentence exists to prevent.

#### Scenario: The dialog is shown

- **WHEN** the setup dialog is shown
- **THEN** it carries the explanation that the files are hosted on a service that throttles them

### Requirement: The setup dialog cannot be dismissed, and Quit is the way out

The setup dialog SHALL NOT be dismissible: clicking outside it, pressing Escape and any close control SHALL
leave it in place. The application cannot be used until initialization has finished, so a dismissed dialog
would leave a user in front of an application that looks ready and does nothing.

It SHALL offer Quit, which SHALL exit the application.

#### Scenario: Trying to dismiss it

- **WHEN** the user clicks outside the setup dialog or presses Escape
- **THEN** the dialog stays where it is

#### Scenario: Quitting

- **WHEN** the user chooses Quit in the setup dialog
- **THEN** the application exits

### Requirement: The dialog goes away when the application is ready

The setup dialog SHALL be taken down as soon as initialization succeeds, leaving the application window.

#### Scenario: Setup finishes

- **WHEN** every component has been installed and initialization succeeds
- **THEN** the setup dialog is taken down and the application is usable

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

### Requirement: The failure dialog keeps every component's outcome and points at the one that stopped

The failure dialog SHALL list the same components the setup dialog listed, in the same order and of the same
length, each still carrying its name and its published size.

Each row SHALL state that component's outcome: that it was installed, that it was never reached, or that it is
the one that stopped. The component that stopped is the one that was working when the failure arrived; there
SHALL be at most one, and it SHALL be marked as the failure rather than as work still in progress.

Whether a component finished SHALL be decided by how far it got and not by the phase it last reported. A
component that installs comes to rest **in a working phase** - the library lands its bar on the end and re-emits
whatever phase it was in - and only a component that was already on disk before the launch ever reports itself
installed. A dialog that read the phase alone would mark the first component that succeeded as the one that
stopped, and the one that actually stopped as never reached.

A component that was installed, and one that was never reached, SHALL be named, worded, iconed and coloured
exactly as the setup dialog names, words, icons and colours them - these are the same facts about the same
component, and only the row that stopped is the failure dialog's own state. A row that renamed itself or changed
its icon because the dialog around it changed would read as a state the component was never in.

No row SHALL show a progress track: a track is a measure of work under way, and no work is under way.

The **overall** progress SHALL be kept. How much of what this machine needs is on disk has the same answer it
had a moment before the failure, and with the failing component's own track gone it is the only thing still
saying how far the attempt got.

The header SHALL also say what became of the attempt: how many components finished, that one stopped part way,
and - where the failure left components behind it - that those were never reached. This is not the bar's
statement repeated. The bar says how much of what this machine needs is on disk; this says what the try itself
came to, and the untouched rows under a failure are the part of it nothing else on screen accounts for. The
counts SHALL be figures rather than words, because a component list can include models and is not of a bounded
length any set of number words could cover.

Where the failure belongs to no component the header SHALL carry no such sentence, there being no component that
stopped to describe. The header SHALL reserve the space either way, so that the dialog does not change height
with the shape of the failure.

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

#### Scenario: A failure after an earlier component downloaded successfully

- **WHEN** one component was already on disk, a second downloaded and expanded to the end, and a third stopped
  while it was transferring
- **THEN** the third is the row marked as having stopped, and the second states that it is installed - though
  the phase it last reported was the one it was expanding in

#### Scenario: A failure that belongs to no component

- **WHEN** initialization fails when no component was transferring - because every component was already
  installed, or because the failure was raised before any of them was reached
- **THEN** no row is marked as having stopped, and every row states the outcome it last reported
- **AND** the header says only what the application needs, with no account of a component that stopped

#### Scenario: The header accounts for the components behind the failure

- **WHEN** a failure arrives with components the attempt never reached
- **THEN** the header states how many finished, that one stopped part way, and that the rest were not reached

#### Scenario: The header on a plan of one component

- **WHEN** the only component this machine needs is the one that stopped
- **THEN** the header says that it stopped part way, without a count of one out of one

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

### Requirement: A machine that supports TensorRT is asked once whether to use it

Once initialization has succeeded, if the provider report says this machine offers TensorRT and the user has never
answered the question, the application SHALL ask whether to enable TensorRT.

The question SHALL say what TensorRT is, that its first run of a model spends minutes optimizing it and later runs are
much faster, and that the answer can be changed at any time in the settings surface, under the AI processor. It SHALL
offer exactly two answers, **No** and **Yes**, with Yes as the primary one.

The question SHALL NOT appear while the setup dialog or its failure state is on screen, and SHALL NOT appear on a
launch whose initialization failed. It follows a retry that succeeds exactly as it follows a first attempt that does.

The question SHALL NOT be dismissable without an answer: pressing Escape or clicking outside it SHALL leave it in
place, and it SHALL carry no close control. An unanswered question is one the application would have to ask again,
which is the repetition this question exists to avoid.

Answering SHALL take the question down, and the question SHALL NOT be asked again on any later launch - see
`gui-settings` for what each answer records.

A machine whose report does not offer TensorRT SHALL never be asked, whatever else it offers.

#### Scenario: A first launch on a machine with an RTX card

- **WHEN** initialization succeeds on a machine reporting TensorRT supported, and the question has never been answered
- **THEN** the TensorRT question is shown once the setup dialog has gone away

#### Scenario: A launch after the question was answered

- **WHEN** initialization succeeds on a machine reporting TensorRT supported, and the question was answered on an
  earlier launch
- **THEN** the question is not shown

#### Scenario: A machine without TensorRT

- **WHEN** initialization succeeds on a machine whose report does not offer TensorRT
- **THEN** the question is not shown

#### Scenario: A launch whose setup failed

- **WHEN** initialization fails on a machine with an RTX card
- **THEN** the failure dialog is shown and the question is not

#### Scenario: A retry that succeeds

- **WHEN** a failed initialization is retried and succeeds on a machine reporting TensorRT supported, and the question
  has never been answered
- **THEN** the question is shown once the setup dialog has gone away

#### Scenario: Trying to dismiss the question

- **WHEN** the question is shown and the user presses Escape or clicks outside it
- **THEN** the question stays on screen
