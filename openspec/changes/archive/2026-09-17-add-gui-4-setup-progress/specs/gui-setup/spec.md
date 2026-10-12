## Purpose

What the graphical application does between being launched and being ready to use: the order its startup
steps run in, that it begins without being asked, what it tells the user about the components this machine
needs while they are being put in place, and the way out of a launch that has not finished.

## ADDED Requirements

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

Where initialization fails, the setup dialog SHALL remain shown, with every row left showing what it last
reported - so the component that stopped, and how far it had got, stay on screen. The failure SHALL be
written to the application's log.

The application SHALL NOT be left usable behind the dialog, because nothing in it can run without the
components that did not arrive, and Quit SHALL remain the way out.

#### Scenario: A component fails to install

- **WHEN** initialization fails part way through
- **THEN** the setup dialog stays shown with the rows it had, and the failing component's row still shows how
  far it got
- **AND** the failure is recorded in the application's log
- **AND** Quit still exits the application
