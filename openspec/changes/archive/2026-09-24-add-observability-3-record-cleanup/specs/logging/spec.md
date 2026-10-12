## MODIFIED Requirements

### Requirement: A unit of work is accounted for from beginning to end

The existing requirements fix the file, the divider, the fields and the levels — the shape of the
log. This one fixes its contents: an application that ran work SHALL leave an account of it, not only
of having started.

Enhancing an image, and running an analysis whose result is not a picture, SHALL each be recorded when
it begins, naming what was asked for and the execution provider requested. Each SHALL be recorded again
when it ends, saying whether it succeeded, failed or was stopped, and how long it took. The closing
record SHALL name the photograph it was about, so a reader with only that record can say which run it
closes.

A run whose operation failed SHALL say which operation failed, and the reason, in the run's own
closing record. The run SHALL NOT also write a separate record for the same failure.

A run refused before any work is attempted SHALL be recorded with what was refused and why. The refusal
SHALL be the only record of that run: it SHALL NOT also be recorded as a run that began and failed.

All of these records SHALL be written at the default verbosity, so that the account of a run is in the
file a user attaches without their having been asked to reproduce it with the environment changed.

#### Scenario: An enhancement is run

- **WHEN** an image is enhanced while a sink is installed
- **THEN** a record at the default verbosity says the run began, what it was asked to apply, and on
  which execution provider
- **AND** a record at the default verbosity says it finished, which photograph it was about, and how
  long it took

#### Scenario: An enhancement fails

- **WHEN** an enhancement fails part of the way through
- **THEN** one record at warning severity says the run failed, identifies the operation that failed,
  and gives the reason it gave
- **AND** no second record at warning severity or above is written by the run for the same failure

#### Scenario: A chain is refused before it runs

- **WHEN** a requested chain names something the application cannot serve and is refused
- **THEN** a record names what was refused and why
- **AND** it is present whether or not the caller reports the refusal to a user
- **AND** no record says the run began or that it failed

#### Scenario: An analysis is refused before it runs

- **WHEN** an analysis names an operation the application cannot serve and is refused
- **THEN** a record at warning severity names the operation and why it was refused
- **AND** no record says the analysis began or that it failed

#### Scenario: An analysis is run

- **WHEN** an analysis whose result is not a picture is run while a sink is installed
- **THEN** a record at the default verbosity says it began, naming the operation and the requested
  execution provider
- **AND** a record at the default verbosity says how it ended, which photograph it was about, and how
  long it took

### Requirement: A run that did not get what it asked for says so

A request that runs on something other than the execution provider it asked for still produces the
right image, and the only symptom is that it took several times longer. That SHALL be recorded at
warning severity, naming both what was asked for and what actually ran it, because it is the answer
to the question a bug report about speed is really asking.

Building the means to run a model — including installing it, where that is needed — SHALL be
recorded, with what it cost, so that a first run that spends minutes compiling is distinguishable
from one that hung. A build that fails SHALL be recorded at warning severity, naming the model, the
execution provider, how long it took and the reason. A build that is stopped rather than failed
SHALL be recorded as stopped.

When several requests wait on one build, the build SHALL be recorded once, not once per request.

#### Scenario: A provider cannot run the model

- **WHEN** the execution provider resolved for a request cannot be used for it and the request is
  served another way
- **THEN** a record at warning severity names the provider that was asked for and the one that ran it

#### Scenario: A model is made ready

- **WHEN** a model that was not ready is made ready to run
- **THEN** a record says which model, on which execution provider, and how long it took

#### Scenario: A model that is already ready

- **WHEN** a request is served by a model that is already ready at raised verbosity
- **THEN** a record says so
- **AND** at the default verbosity no record is written for it

#### Scenario: A model cannot be made ready

- **WHEN** a model is on disk and the execution provider being attached, including the CPU, will not
  open it
- **THEN** a record at warning severity names the model, the provider, how long the attempt took, and
  the reason

#### Scenario: Several requests wait on a build that fails

- **WHEN** several requests are waiting on one build of a model and the build fails
- **THEN** exactly one record says that build failed

### Requirement: An installation says what it transferred

An installation is the longest thing the application does and the most exposed to a user's network.
Each dependency and each model SHALL be recorded when it is found already present, when its
installation begins, and when it is ready. The record that it is ready SHALL say how long the
installation took.

A transfer that has to be repeated, and one that ends in failure, SHALL be recorded at warning
severity naming the artifact, so that a bug report about an install that never finishes says which
artifact it was on. An installation that was stopped, because nothing was left waiting for it or
because the application was shutting down, SHALL NOT be recorded as a failure.

#### Scenario: A dependency is already present

- **WHEN** an installation finds a dependency already present and complete at raised verbosity
- **THEN** a record says so and nothing is transferred

#### Scenario: An installation runs

- **WHEN** a dependency or model is installed
- **THEN** a record at the default verbosity says the installation began and names the artifact
- **AND** a record at the default verbosity says it is ready and how long it took

#### Scenario: A transfer has to be repeated

- **WHEN** what arrived does not match what was expected and the transfer is attempted again
- **THEN** a record at warning severity names the artifact and says why it is being repeated

#### Scenario: An installation fails

- **WHEN** an installation ends in failure
- **THEN** a record at warning severity names the artifact and the reason

#### Scenario: An installation is interrupted by shutdown

- **WHEN** the application shuts down while an installation is under way
- **THEN** no record at warning severity or above says the installation failed

## ADDED Requirements

### Requirement: Work that was stopped is not recorded as a failure

A user who cancels a run, a run displaced by a newer one, and work abandoned because the application
is shutting down have all asked for the stop, or at least caused it. None of them is a fault. Recording
them as failures would fill every file and every collector with failures from users who changed their
minds, and would bury the real ones.

A unit of work whose account began at the default verbosity SHALL, when it is stopped, end with a
record at the default verbosity that says it stopped. That record SHALL say whether the stop was a
cancellation or a shutdown, and how long the work had run. It SHALL be written below warning severity,
and nothing the unit writes about the stop SHALL say it failed.

This holds for enhancements, analyses, automatic analyses, initialization, and installations.

#### Scenario: A user cancels an enhancement

- **WHEN** an enhancement is cancelled by its caller part of the way through
- **THEN** a record at the default verbosity says the run stopped because it was cancelled, and how
  long it had run
- **AND** no record at warning severity or above is written about that run

#### Scenario: The application shuts down during an analysis

- **WHEN** an analysis is abandoned because the application is shutting down
- **THEN** a record says the analysis stopped because of a shutdown
- **AND** no record says it failed

#### Scenario: An automatic analysis is withdrawn

- **WHEN** an automatic analysis of a photograph is cancelled before it concludes
- **THEN** a record at the default verbosity names the photograph and says the analysis stopped

### Requirement: Initialization is accounted for from beginning to end

Initialization is the first thing every application does and the one whose failure leaves the least
behind: a window that never appears, or a command that exits. It SHALL be recorded when it begins, and
again when it ends. The closing record SHALL say whether it succeeded, how long it took, and what the
machine turned out to support.

A failed initialization SHALL be recorded at warning severity with how long it ran and the reason,
whether or not the application goes on to report the failure itself. Initialization refused because
another process of the same application already holds the configuration directory is an ordinary
outcome, not a fault. It SHALL be recorded below warning severity, naming the holder where one is
known.

Loading the inference runtime SHALL be recorded with how long it took, since it is the single slowest
step of an initialization that has nothing to install.

#### Scenario: Initialization succeeds

- **WHEN** an application initializes while a sink is installed
- **THEN** a record at the default verbosity says initialization finished, how long it took, and which
  execution providers the machine supports

#### Scenario: Initialization fails

- **WHEN** initialization fails, for instance because a dependency cannot be installed or the runtime
  cannot be loaded
- **THEN** a record at warning severity says initialization failed, how long it ran, and why

#### Scenario: Another process holds the claim

- **WHEN** initialization is refused because another process holds the configuration directory
- **THEN** a record below warning severity says it was refused and names the holder where known
- **AND** no record at warning severity or above is written about it

#### Scenario: The runtime is loaded

- **WHEN** the inference runtime is loaded during initialization
- **THEN** a record names the library that was loaded and how long loading it took

### Requirement: A failure is recorded once, where it happened

A failure recorded several times over reads, in a file and more so at a collector, as several
failures. The layer that returns a failure is the one that knows what was being attempted, so a
failure SHALL be recorded at warning severity by that layer, once.

A unit of work that fails because the work it contains failed SHALL still record its own outcome,
since its duration and subject are its own. It SHALL NOT write a second record for its own outcome.

A front end that receives a failure this library has already recorded SHALL NOT record it again, at
any severity. What only the front end knows about it, such as which of its requests it was, belongs in
the context the library's record is emitted within rather than in a second record. A front end SHALL
still record, at a level of its own choosing, the failures only it can observe.

#### Scenario: An operation fails inside an enhancement

- **WHEN** an enhancement run from the graphical application fails in one of its operations
- **THEN** exactly one record at warning severity or above in the file describes that failure of the
  run

#### Scenario: A face signal cannot be read during an automatic analysis

- **WHEN** the face signal of an automatic analysis fails
- **THEN** one record at warning severity says the detection failed and why
- **AND** the record of what the analysis concluded names the signal that could not be read and the
  reason
- **AND** no other record at warning severity or above is written for it

#### Scenario: A photograph cannot be opened in the graphical application

- **WHEN** a photograph the user opened cannot be decoded
- **THEN** exactly one record at warning severity or above says so, naming the file and the reason

### Requirement: Reading and writing an image records its failure

Opening, describing, fingerprinting, encoding and saving an image are where a user's own files meet
this application. A failure at any of them SHALL be recorded at warning severity, whichever application
asked, naming what was being done, the file (or, for an encode, the format), and the reason.

A success SHALL NOT be recorded at the default verbosity: describing a folder of photographs is one
call per file, and the file's size is bounded by what was done rather than by how many files were
chosen.

#### Scenario: A file that is not an image

- **WHEN** an application asks this library to open a file that cannot be decoded as an image
- **THEN** a record at warning severity names the file, says it was being opened, and gives the reason

#### Scenario: A save that cannot be written

- **WHEN** a result is saved to a destination that cannot be written
- **THEN** a record at warning severity names the destination and the reason

#### Scenario: A folder of readable photographs

- **WHEN** an application describes many readable photographs at the default verbosity
- **THEN** no record is written for any of them

### Requirement: Severity says what happens next

A reader, and more so a query over many installations, reads severity as a statement about the
application rather than about the record. The two highest severities SHALL therefore each mean one
thing in this library's records:

- **Error** SHALL be used only for what the process does not survive: a panic.
- **Warning** SHALL be used for a failure, and for a degradation the application continues through that
  costs the user something they did not choose.

A failure this library returns to its caller SHALL be recorded at warning severity even when the caller
treats it as fatal, because whether it is fatal is the caller's decision.

#### Scenario: A model with no published source

- **WHEN** a model no published source names is requested and its installation is refused
- **THEN** the refusal is recorded at warning severity, not error

#### Scenario: No run cache can be opened

- **WHEN** neither the disk nor the memory run cache can be opened and the application continues
  without one
- **THEN** a record at warning severity says every operation will be recomputed

#### Scenario: An ordinary session

- **WHEN** an application runs without anything panicking
- **THEN** no record from this library in the file is at error severity
