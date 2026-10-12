## ADDED Requirements

### Requirement: A unit of work is accounted for from beginning to end

The existing requirements fix the file, the divider, the fields and the levels — the shape of the
log. This one fixes its contents: an application that ran work SHALL leave an account of it, not only
of having started.

Enhancing an image SHALL be recorded when it begins, naming what was asked for, and again when it
ends, saying whether it succeeded and how long it took. A run refused before any work is attempted
SHALL be recorded with what was refused and why.

Both records SHALL be written at the default verbosity, so that the account of a run is in the file
a user attaches without their having been asked to reproduce it with the environment changed.

#### Scenario: An enhancement is run

- **WHEN** an image is enhanced while a sink is installed
- **THEN** a record at the default verbosity says the run began and what it was asked to apply
- **AND** a record at the default verbosity says it finished and how long it took

#### Scenario: An enhancement fails

- **WHEN** an enhancement fails part of the way through
- **THEN** a record identifies the operation that failed and the reason it gave
- **AND** the run's own outcome is recorded as a failure rather than left to be inferred from the
  absence of a completion

#### Scenario: A chain is refused before it runs

- **WHEN** a requested chain names something the application cannot serve and is refused
- **THEN** a record names what was refused and why
- **AND** it is present whether or not the caller reports the refusal to a user

### Requirement: A result that did not come from a model says so

A cached result is indistinguishable from a computed one in everything a user sees except the time it
took, which is the whole point of it and also what makes a cache that is silently doing nothing
impossible to notice. Whether each step of a run was served from the store or computed SHALL be
recorded.

These records repeat per operation, so they SHALL be written below the default verbosity — available
to a user who has been asked for more detail, absent from an ordinary session's file.

#### Scenario: A step is served from the store

- **WHEN** a step of a run is served from the image cache at raised verbosity
- **THEN** a record identifies the operation and says it was served from the store

#### Scenario: A step is computed

- **WHEN** a step of a run is not in the store and is computed at raised verbosity
- **THEN** a record identifies the operation and says it was computed

#### Scenario: An ordinary session

- **WHEN** a run is made at the default verbosity
- **THEN** the per-step records are absent from the file
- **AND** the run's own beginning and outcome are still present

### Requirement: A store that is not storing says so where it was decided

A result the store refused to keep and a result it failed to keep are the same silence to a caller,
and a run whose every write is dropped caches nothing while behaving exactly as if it were working.
The two SHALL be distinguished in the log, at the point the outcome is known.

A write the store declined — because it is a store that keeps nothing, or the entry would not
outlive the run — SHALL be recorded below the default verbosity, since it is ordinary. A write or a
read that **failed**, and an entry that was found but could not be read back, SHALL be recorded at
warning, since each of them means work is being redone for a reason a user did not choose.

#### Scenario: A write the store declines

- **WHEN** the store declines to keep a result at raised verbosity
- **THEN** a record says the result was not kept and that the store declined it

#### Scenario: A write that fails

- **WHEN** keeping a result fails
- **THEN** a record at warning severity says so and identifies the run it belongs to

#### Scenario: An entry that cannot be read back

- **WHEN** a stored result is found for a step and cannot be read back as an image
- **THEN** a record at warning severity says so
- **AND** the step is recorded as having been computed instead

### Requirement: A run that did not get what it asked for says so

A request that runs on something other than the execution provider it asked for still produces the
right image, and the only symptom is that it took several times longer. That SHALL be recorded at
warning severity, naming both what was asked for and what actually ran it, because it is the answer
to the question a bug report about speed is really asking.

Building the means to run a model — including installing it, where that is needed — SHALL be
recorded, with what it cost, so that a first run that spends minutes compiling is distinguishable
from one that hung.

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

### Requirement: Work that is about to be redone is recorded with its reason

Compiled artifacts are discarded whenever what they were compiled against changes, and recompiling
them costs a user minutes with no visible cause. Discarding them SHALL be recorded, naming what
changed.

Releasing what is held in memory — because it has gone unused, or because the application asked for
it — SHALL also be recorded, so that a session that gets slower over a long run has an account of
what it gave up and when.

#### Scenario: Compiled artifacts are discarded

- **WHEN** artifacts compiled for a previous version of the inference runtime are discarded
- **THEN** a record says they were discarded and names the version that replaced it

#### Scenario: Something held in memory is released

- **WHEN** a model held in memory is released, whether for disuse or on request
- **THEN** a record identifies what was released and why

### Requirement: An installation says what it transferred

An installation is the longest thing the application does and the most exposed to a user's network.
Each dependency and each model SHALL be recorded when it is found already present, when its
installation begins, and when it is ready.

A transfer that has to be repeated, and one that ends in failure, SHALL be recorded at warning
severity naming the artifact, so that a bug report about an install that never finishes says which
artifact it was on.

#### Scenario: A dependency is already present

- **WHEN** an installation finds a dependency already present and complete at raised verbosity
- **THEN** a record says so and nothing is transferred

#### Scenario: An installation runs

- **WHEN** a dependency or model is installed
- **THEN** a record at the default verbosity says the installation began and names the artifact
- **AND** a record at the default verbosity says it is ready

#### Scenario: A transfer has to be repeated

- **WHEN** what arrived does not match what was expected and the transfer is attempted again
- **THEN** a record at warning severity names the artifact and says why it is being repeated

#### Scenario: An installation fails

- **WHEN** an installation ends in failure
- **THEN** a record at warning severity or above names the artifact and the reason

### Requirement: A run working from a fallback source says so

Where the application resolves something from more than one source in preference order — a live
listing of what is published, a copy kept from a previous run, or what was compiled into the build —
the source it ended up using SHALL be recorded whenever it was not the first choice.

A run working from a listing compiled in months ago behaves almost identically to one working from
the current listing, right up to the point where it cannot find a model that exists. The log is where
that is distinguishable.

#### Scenario: The live source is unreachable

- **WHEN** the live listing cannot be read and a copy kept from a previous run is used
- **THEN** a record at warning severity says the live listing failed and that the kept copy is in use

#### Scenario: Nothing but the build's own copy is available

- **WHEN** neither the live listing nor a kept copy can be used
- **THEN** a record at warning severity says the listing compiled into the build is in use

#### Scenario: The live source is used

- **WHEN** the live listing is read successfully at raised verbosity
- **THEN** a record says so and how many entries it named

### Requirement: The file's size is bounded by what was done, not by how big the picture was

A record on a path that repeats per tile, per region or per pixel would make an ordinary session's
file unreadable and would put the cost of a write on a loop whose speed is the application's speed.
At the default verbosity, the number of records a unit of work produces SHALL NOT grow with the size
of the image it was given.

This bounds the file for the same reason the sink writes synchronously rather than buffering: a
record costs a write on the thread that emitted it, and that price is worth paying only for records
a reader will actually want.

#### Scenario: A large image and a small one

- **WHEN** the same enhancement is run at the default verbosity over a small image and over one many
  times larger
- **THEN** both produce the same number of records

#### Scenario: A chain of several operations

- **WHEN** a run applies several operations at the default verbosity
- **THEN** the records it produces are bounded by the number of operations rather than by the work
  each one did
