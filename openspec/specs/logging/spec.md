# logging Specification

## Purpose

Every application of this project keeps a durable account of what it did, in one file a user can be
asked to attach to a bug report. This capability covers where that file is, what a reader finds in
it, which sources reach it — this library, the front ends, and the inference runtime's own
diagnostics — how it is kept from growing without bound, and what happens when it cannot be written
at all.

It covers both halves of that, which are separate concerns and are kept apart below. The **shape**
of the file: its location, the divider between sessions, the fields every record carries, the levels
that are kept, and the rotation that bounds a week of history. And its **contents**: that a unit of
work is accounted for from beginning to end, that a result which was served rather than computed or
which ran on something other than what was asked for says so where that was decided, and that an
installation says what it transferred. The two bounds on size are likewise distinct — rotation
bounds the file over *days*, while the contents are bounded per run by what was asked for rather
than by how large the picture was.

## Requirements

### Requirement: Records are written to one file in the configuration directory

An application SHALL write its records to `logs/opai.log` beneath the configuration directory its
application name resolves to. The file SHALL be appended to rather than replaced, so a run does not
destroy the account of the run before it.

The path SHALL be derivable from the application name alone, without installing the sink, so that an
interface can tell a user where the file is whether or not this process is logging to it.

The name `opai.log` is fixed. It is what a user is asked to attach, so rotation MUST NOT be allowed
to move the live records to a differently-named file.

#### Scenario: An application installs the sink

- **WHEN** an application installs the log sink under an application name
- **THEN** `logs/opai.log` exists beneath that name's configuration directory
- **AND** the directories it needs are created if they were absent

#### Scenario: A second run appends

- **WHEN** an application installs the sink and a file from a previous run is already present
- **THEN** the previous run's records are still in the file afterwards
- **AND** this run's records follow them

#### Scenario: Locating the file without logging

- **WHEN** the path of the log file is requested for an application name
- **THEN** it is reported
- **AND** no sink is installed, no record is written, and nothing is created by asking

### Requirement: Each session is separated from the one before it

Because one file holds every run, a reader SHALL be able to tell where one run ends and the next
begins. Before its first record, a session SHALL write a divider line of three hyphens and nothing
else, then a header record naming the application, its version and the operating system and
architecture it is running on.

The divider is written even when the previous session ended abruptly, since that is the case a
reader most needs to find.

#### Scenario: A session starts

- **WHEN** an application installs the log sink
- **THEN** a line containing only `---` is appended
- **AND** the next record identifies the application, its version, its operating system and its
  architecture

#### Scenario: Two runs in the same file

- **WHEN** two applications have run in turn under the same application name
- **THEN** the file contains two dividers, each followed by that run's header

### Requirement: Every record identifies the application that produced it

Three binaries write to one file, and slice 1's claim does not exclude every overlap: a process
refused by it has already started logging by the time it is refused. Every record SHALL therefore
carry the application that produced it, spelled exactly as a single-instance refusal spells it.

The spelling is the identity the process declared rather than one of a fixed set. This project's own
binaries declare `gui`, `cli` and `perf`, and their records SHALL carry those spellings unchanged; an
application embedding this library carries its own. One identity backs both the log field and the
single-instance record, so the two can never disagree about what a process is called.

This SHALL hold for every record in the file, including the ones produced by the inference runtime
rather than by this project's own code.

#### Scenario: A record is written

- **WHEN** any record is written by any of the three applications
- **THEN** it carries the application's name as a field
- **AND** the spelling is the same one a single-instance refusal uses for that application

#### Scenario: A record written by an embedding application

- **WHEN** a record is written by a process that declared its own identity
- **THEN** it carries that identity as the field, spelled as declared

#### Scenario: Two applications in one file

- **WHEN** records from two different applications are present in the file
- **THEN** each record says which application wrote it, independently of which session it belongs to

### Requirement: The inference runtime's diagnostics are captured

The inference runtime produces its diagnostics outside this project's own code. Those diagnostics
SHALL be recorded in the application's log at the severity the runtime assigned them, and SHALL NOT
be written to the terminal.

A captured diagnostic SHALL retain what identifies it: the severity, the location in the runtime's
source that produced it, and the identity of the session or environment it belongs to, where the
runtime supplies one.

Capturing them SHALL NOT change which diagnostics the runtime produces; the level at which the
runtime reports is `initialization`'s decision and is unchanged by this capability.

#### Scenario: The runtime reports a warning

- **WHEN** the inference runtime emits a warning while a sink is installed
- **THEN** it appears in the log file as a record at warning severity
- **AND** it does not appear on the process's terminal output

#### Scenario: What a captured diagnostic carries

- **WHEN** a diagnostic from the runtime is recorded
- **THEN** it carries the location in the runtime's source that produced it
- **AND** it carries the runtime's own identifier for the session or environment, when the runtime
  supplied one

### Requirement: The file is rotated daily and a week of history is kept

The log SHALL be rotated once per day, and rotation SHALL leave the live records in `opai.log`. A
rotated file SHALL be compressed and SHALL carry the date it covers in its name.

At most seven rotated files SHALL be kept; older ones SHALL be deleted as newer ones are made, so a
long-lived installation does not grow without bound.

Rotation SHALL NOT depend on the application having been running when the day changed. A process
that starts while the live file's last record is from an earlier day SHALL rotate it before writing,
so a session's records are never mixed into another day's file.

#### Scenario: The day changes while the application runs

- **WHEN** an application is running and the day changes
- **THEN** the records written after it go to `opai.log`
- **AND** the records written before it are in a compressed file named for the day they cover

#### Scenario: The application starts on a later day

- **WHEN** an application installs the sink and the live file's last write was on an earlier day
- **THEN** that file is rotated before this session's divider is written
- **AND** this session's records are the first in a new `opai.log`

#### Scenario: History is bounded

- **WHEN** rotation would leave more than seven rotated files
- **THEN** the oldest are deleted until seven remain

### Requirement: Records survive an abrupt end

A record SHALL be handed to the operating system before the call that emitted it returns. Records
already emitted SHALL therefore be readable in the file after the process is killed, crashes, or
dies inside native code, without any later run being needed to recover them.

This is the property the log is for: the records that matter most are the last ones before a failure,
and a sink that buffers them in the process loses exactly those.

#### Scenario: The process is killed

- **WHEN** a process emits records and is then killed without being allowed to shut down
- **THEN** every record it emitted is in the file

#### Scenario: A crash inside the inference runtime

- **WHEN** the inference runtime reports a diagnostic and the process then dies inside native code
- **THEN** that diagnostic is in the file
- **AND** finding it there does not require starting the application again

### Requirement: A panic is recorded before the process ends

An application panic SHALL be recorded, carrying the message and the location that raised it, before
the process's default panic handling runs. Reporting it SHALL NOT change what the panic does: the
process still unwinds or aborts exactly as it would have.

#### Scenario: Application code panics

- **WHEN** a panic is raised while a sink is installed
- **THEN** a record at error severity carries its message and the source location that raised it
- **AND** the panic itself proceeds as it would have without the sink

### Requirement: Informational records and above are kept, and the level can be changed for a run

The sink SHALL record at informational severity and above by default. A single run SHALL be able to
raise or lower that through the environment, so a user chasing a problem can be asked for more
detail without a new build.

Diagnostics from the inference runtime SHALL be filterable independently of this project's own
records, because the runtime's verbose levels produce a per-node flood that would bury everything
else.

#### Scenario: Default verbosity

- **WHEN** an application installs the sink with nothing set in its environment
- **THEN** informational, warning and error records are written
- **AND** debug and finer records are not

#### Scenario: A run is asked for more detail

- **WHEN** the environment asks for debug severity and the application installs the sink
- **THEN** debug records are written to the file as well

### Requirement: A library consumer that does not ask for logging gets none

Using this library SHALL NOT, on its own, produce output or create a file. Records are emitted
unconditionally by library code and discarded unless a sink has been installed by the application
that owns the process.

#### Scenario: The library is used without a sink

- **WHEN** an application uses the library without installing a sink
- **THEN** no log file is created
- **AND** nothing is written to its terminal output

### Requirement: Failing to install the sink does not stop the application

An application SHALL be able to start without a log. Where the log directory cannot be created or the
file cannot be opened, installing the sink SHALL report that it failed and the application SHALL
continue, unlogged.

Installing the sink SHALL be possible once per process. A second attempt SHALL be refused with an
error rather than silently accepted, since a caller that reached it believes it owns the process's
logging, and SHALL leave the sink already installed working and unchanged.

#### Scenario: The log file cannot be opened

- **WHEN** the log directory or file cannot be created or opened
- **THEN** installing the sink reports the failure
- **AND** the application starts and runs without a log

#### Scenario: A second attempt in one process

- **WHEN** a sink is installed and installing one is attempted again in the same process
- **THEN** the second attempt is refused with an error
- **AND** records continue to reach the file through the sink installed first

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

### Requirement: A result that did not come from a model says so

A cached result is indistinguishable from a computed one in everything a user sees except the time it
took, which is the whole point of it and also what makes a cache that is silently doing nothing
impossible to notice. Whether a run's work was served from the store or computed SHALL be recorded.

This SHALL hold wherever the store is read, rather than on the enhancement path alone: an analysis
whose result is not a picture is served from the same store and is subject to the same silence, and a
record written for one path and not the other would make "the store is doing nothing" answerable for
half the application. A served analysis and a computed one SHALL each be recorded, and both records
SHALL name the operation and the entry, so a run that missed and the later run it serves can be read
as one sequence rather than as two unrelated records.

These records repeat per operation, so they SHALL be written below the default verbosity — available
to a user who has been asked for more detail, absent from an ordinary session's file.

#### Scenario: A step is served from the store

- **WHEN** a step of a run is served from the run cache at raised verbosity
- **THEN** a record identifies the operation and says it was served from the store

#### Scenario: A step is computed

- **WHEN** a step of a run is not in the store and is computed at raised verbosity
- **THEN** a record identifies the operation and says it was computed

#### Scenario: An analysis is served from the store

- **WHEN** a run whose result is not a picture is served from the run cache at raised verbosity
- **THEN** a record identifies the operation and the entry, and says it was served from the store

#### Scenario: An analysis is computed

- **WHEN** a run whose result is not a picture is not in the store and executes its model at raised verbosity
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

### Requirement: Records are also sent to a collector once the application starts telemetry

The file is the account a user can be asked for. A collector is where records arrive without their being asked. Once
an application has started telemetry, the records that reach the file SHALL also be sent to this project's collector
if the build names one: this library's, the front end's and the inference runtime's. Units of work that were
recorded as spans SHALL be sent as traces, and a record emitted inside one SHALL carry that trace.

An application SHALL be able to mark a record as the file's alone. A record so marked SHALL be written to the file as
any other, and SHALL NOT be sent to the collector. This exists for records whose account reaches the collector by
another path, so the collector does not receive the same fact twice.

Sending SHALL NOT change the file. A record SHALL be written to `opai.log` exactly as it would be with telemetry never
started, and a record discarded on the way to the collector SHALL still reach the file. The only records telemetry
adds to the file are its own: that it started, and that sending failed.

#### Scenario: An application starts telemetry

- **WHEN** an application whose build names a collector installs the log sink and then starts telemetry
- **THEN** a record emitted afterwards at informational severity is received by the collector
- **AND** the same record is written to the file

#### Scenario: A record inside a unit of work

- **WHEN** a record is emitted inside a span while telemetry is running
- **THEN** the collector receives the span as part of a trace
- **AND** the record it receives carries that trace's identifiers

#### Scenario: The file is the same either way

- **WHEN** the same work is run once with telemetry started and once without
- **THEN** the records the work produced are written to the file identically in both runs

#### Scenario: A record marked as the file's alone

- **WHEN** an application emits a warning marked as the file's alone while telemetry is running
- **THEN** the warning is written to the file
- **AND** the collector does not receive it

### Requirement: Nothing is sent unless the application starts telemetry and the build names a collector

Using this library, or installing its log sink, SHALL NOT on its own send anything. Records SHALL leave the process
only after the application has explicitly started telemetry. They SHALL NOT leave it at all from a build that was
compiled without a collector's address and credentials.

Starting telemetry in a build without a collector SHALL report that nothing will be sent, and SHALL NOT be a failure:
it is the ordinary state of a development build.

The records SHALL say which kind of build sent them. A release build SHALL report itself as production, and any
other build as development.

#### Scenario: An application that never starts telemetry

- **WHEN** an application installs the log sink and never starts telemetry
- **THEN** no connection is made to any collector
- **AND** its records reach the file as before

#### Scenario: A build without a collector

- **WHEN** an application built without a collector's address starts telemetry
- **THEN** it is told that nothing will be sent
- **AND** no connection is made to any collector
- **AND** the application continues normally

#### Scenario: A development build with a collector

- **WHEN** a build that is not a release build, and names a collector, sends records
- **THEN** the collector receives them marked as coming from a development environment

### Requirement: What is sent has a floor that the environment does not lower

The environment can raise the file's detail for one run. It SHALL NOT change what is sent. The collector SHALL
receive informational records and above from this project, and warnings and above from the inference runtime, whatever
the environment asks the file for.

The inference runtime wraps each of its diagnostics in a context of its own. That context SHALL reach the collector as
fields on the diagnostic, not as a unit of work of its own.

#### Scenario: A run asked for more detail

- **WHEN** the environment asks for debug severity while telemetry is running
- **THEN** debug records are written to the file
- **AND** the collector receives none of them

#### Scenario: A runtime diagnostic is sent

- **WHEN** the inference runtime emits a warning while telemetry is running
- **THEN** the collector receives it as a record carrying the runtime's source location and identifier
- **AND** receives no trace for the context the runtime wrapped it in

### Requirement: Sending costs the application nothing it would notice

A collector that is slow, unreachable or refusing SHALL NOT delay the call that emitted a record, stop a record from
reaching the file, or stop the application. What cannot be sent SHALL be discarded.

A failure to send SHALL be recorded in the file and SHALL NOT be sent to the collector. The first failure in a session
SHALL be recorded at warning severity, so that a session whose telemetry never arrived says so at the default
verbosity. Later failures SHALL be recorded below it, so that a collector that stays unreachable does not fill the file.

#### Scenario: The collector is unreachable

- **WHEN** telemetry is running and the collector cannot be reached
- **THEN** the application runs and writes its file as it would without telemetry
- **AND** one record at warning severity in the file says that sending failed

#### Scenario: The collector stays unreachable

- **WHEN** sending fails repeatedly during one session
- **THEN** only the first failure is recorded at warning severity or above
- **AND** no record of a sending failure is sent to the collector

### Requirement: Stopping telemetry sends what was held and then nothing more

Stopping telemetry SHALL send the records already held, waiting a bounded time for the collector, and SHALL send
nothing after it. Records emitted after it SHALL still reach the file. Stopping SHALL be safe to repeat, and safe to
call when telemetry never started.

Telemetry SHALL be started at most once in a process. A second attempt SHALL be refused with an error, and SHALL leave
the running telemetry, or the stopped one, as it was.

#### Scenario: An application stops telemetry at exit

- **WHEN** an application that started telemetry stops it before exiting
- **THEN** the records emitted before the stop are received by the collector

#### Scenario: Records after a stop

- **WHEN** a record is emitted after telemetry was stopped
- **THEN** it is written to the file
- **AND** it is not sent to the collector

#### Scenario: A second start in one process

- **WHEN** telemetry has been started in a process and starting it is attempted again
- **THEN** the second attempt is refused with an error

### Requirement: A session that starts sending says so

Records emitted before telemetry starts are not sent. So the first record a collector receives from a session SHALL
identify the application's version, its operating system and its architecture, the same facts the file's session
header carries.

#### Scenario: Telemetry starts

- **WHEN** an application starts telemetry in a build that names a collector
- **THEN** the collector receives a record identifying the version, operating system and architecture

### Requirement: Work handed to another thread stays inside the unit of work that handed it off

The library does slow work on threads of its own: decoding and encoding images, building sessions, expanding
archives. A record emitted there SHALL carry the enclosing context of the call that handed the work off, in the file
and at the collector, as it would if the work had run on the calling thread.

#### Scenario: A record from work handed off

- **WHEN** work handed to another thread inside a span emits a record
- **THEN** the record in the file carries that span's fields
- **AND** the record the collector receives belongs to that span's trace

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

### Requirement: Every unit of work reaches the collector as a span

A record says that something happened. A trace says where the time went and what caused what. While telemetry is
running, each unit of work this library does SHALL reach the collector as a span:
- initialization, and loading the inference runtime within it
- installing a dependency or a model
- a request for a model's session, and building that session
- an enhancement, and each operation of it
- an analysis whose result is not a picture, and an automatic analysis
- opening, describing, fingerprinting, encoding and saving an image
- reading from and writing to the run cache

A unit done on behalf of another SHALL be a child of it. So the installs, session builds, operations and cache access
of one enhancement belong to that enhancement's trace, including when the work was handed to another thread. A record
emitted inside a unit SHALL carry that unit's trace.

#### Scenario: An enhancement that installs its model

- **WHEN** an enhancement is run while telemetry is running, and its model has to be installed and built first
- **THEN** the collector receives one trace for the enhancement
- **AND** that trace holds a span for the enhancement, one for each of its operations, and, beneath the operation that
  needed it, a span for the session request, the build and the install

#### Scenario: Describing a photograph

- **WHEN** an application asks for a photograph's description while telemetry is running
- **THEN** the collector receives a span naming that operation

#### Scenario: A failure record inside a unit

- **WHEN** a unit of work fails and records its failure while telemetry is running
- **THEN** the record the collector receives carries the trace of that unit's span

### Requirement: A unit's span says how it ended

A unit's span SHALL say whether the unit finished, stopped or failed, matching the record that closed it. A failed
unit's span SHALL be marked as failed, carrying the reason, so a trace view shows it failed without its records being
read. A stopped unit's span SHALL NOT be marked as failed, for the same reason its record is not a warning.

#### Scenario: A failed enhancement

- **WHEN** an enhancement fails part of the way through while telemetry is running
- **THEN** the enhancement's span says it failed and is marked as failed with the reason
- **AND** the span of the operation that failed is marked as failed too

#### Scenario: A cancelled enhancement

- **WHEN** an enhancement is cancelled by its caller while telemetry is running
- **THEN** the enhancement's span says it stopped
- **AND** it is not marked as failed

### Requirement: Requests that wait on one build are linked to it

Several requests may wait on one build of a model, as the file already records. The build SHALL be one span, in the
trace of the request that started it. Every other request that waited on it SHALL be linked to that span, so their
traces show the time spent waiting and where it went, without a second build that did not happen.

#### Scenario: Two enhancements need the same model at once

- **WHEN** two enhancements request the same model while it is not ready, and telemetry is running
- **THEN** the collector receives exactly one span for building that model
- **AND** the request span of the enhancement that did not start the build carries a link to it

### Requirement: The file does not carry the collector's spans

The spans sent to the collector SHALL NOT change the file. A record SHALL be written to `opai.log` with exactly the
fields it would have if those spans did not exist: no field of an enclosing unit's span is added to it. Only the
contexts that existed in the file before, such as the model a runtime diagnostic was emitted while building, SHALL
decorate the records inside them.

There is one other context the file SHALL carry: the request a graphical application's window made. A record emitted
while that request is served SHALL carry the request's identifier when the request has one, so that a failure the
library recorded can be traced back to the window's request. The identifier SHALL appear on a record once, however
many layers wrote records while serving that request. No other field of the request's context SHALL be added to a
record.

#### Scenario: A record inside an enhancement

- **WHEN** an enhancement is run at the default verbosity
- **THEN** each of its records in the file carries only the fields it was emitted with, the contexts the file already
  wrote before this change, and the identifier of the window's request that asked for it

#### Scenario: A runtime diagnostic during a build

- **WHEN** the inference runtime emits a warning while a model's session is being built
- **THEN** the record in the file still names the model and the execution provider being attached

#### Scenario: A failure the library recorded for the window

- **WHEN** an enhancement the window asked for under a given run fails in one of its operations
- **THEN** the library's record of the failure in the file carries that run's identifier exactly once

### Requirement: Spans are bounded by what was done, not by how big the picture was

The number of spans a unit of work produces SHALL NOT grow with the size of the image it was given, for the same reason
the number of its records does not. There SHALL be no span per tile, per region or per pixel.

#### Scenario: A large image and a small one

- **WHEN** the same enhancement is run with telemetry running over a small image, and again over one many times larger
- **THEN** both send the same number of spans

### Requirement: The work is measured while telemetry runs

The collector SHALL receive measurements that answer, across every installation that sends, questions no single record
does:
- how long each kind of unit takes, and how often it finishes, stops or fails
- how long each model's operation takes on the execution provider that actually ran it
- how often the run cache serves a result, and how often it misses
- how often a requested execution provider could not be used and another ran the work instead, by the provider that
  was attempted
- how many failures of each kind each kind of unit has
- how many models are held in memory, by execution provider

Measurements SHALL be sent under the same rules as records: only once telemetry has started, only from a build that
names a collector, and nothing after it stops. Each measurement SHALL be identified only by values from small, fixed
sets: a kind of unit, an outcome, a model, an execution provider, a kind of failure. No path, photograph identity,
parameter value or error message SHALL identify a measurement.

#### Scenario: A finished enhancement is measured

- **WHEN** an enhancement finishes while telemetry is running
- **THEN** the collector receives its duration, identified as an enhancement that finished
- **AND** the duration of each operation it computed, identified by the model and the execution provider that ran it

#### Scenario: A step served from the store

- **WHEN** a step of an enhancement is served from the run cache while telemetry is running
- **THEN** the collector's count of cache hits for steps grows by one

#### Scenario: A provider falls back

- **WHEN** the execution provider requested for a model cannot open it and the CPU runs it instead, while telemetry is
  running
- **THEN** the collector's count of fallbacks from that requested provider grows by one

#### Scenario: A failure is counted by kind

- **WHEN** an enhancement fails while telemetry is running
- **THEN** the collector's count of enhancement failures of that error's kind grows by one
- **AND** the kind is one of a fixed set of names, never the error's text

#### Scenario: An application that never starts telemetry

- **WHEN** an application runs work and never starts telemetry
- **THEN** no measurement leaves the process

### Requirement: A graphical application's requests are units of work

Each request a graphical application's window makes of it SHALL be a unit of work, sent to the collector as a span
while telemetry is running. The units of work this library does to serve a request SHALL be its children, including
work handed to another thread. So one enhancement the window asked for is one trace.

Where that trace begins SHALL depend on the window:
- **When the request names a trace the window started,** the request's span SHALL continue that trace, as a child of
  the span the request names. So the window's work and the backend's are one trace, rooted in the window.
- **When the request names no trace, or names one that cannot be read,** the request's span SHALL root a new trace,
  and the request SHALL be served exactly as it would otherwise be. A trace the window named badly SHALL NOT fail
  the request.

What the request names SHALL NOT reach the file. The file SHALL carry the request's identifier as before, and nothing
else of the request's context.

A request's span SHALL say whether the request finished, stopped or failed. A failed request's span SHALL be marked as
failed, carrying the reason. A stopped one SHALL NOT be.

The one request exempt is the one that records the user's analytics choice. Its whole contract is that turning
analytics off is not reported anywhere.

#### Scenario: An enhancement from the window

- **WHEN** the window asks for an enhancement while telemetry is running, naming no trace
- **THEN** the collector receives one trace whose root is the window's request
- **AND** the enhancement's span is a child of that request

#### Scenario: An enhancement in a trace the window started

- **WHEN** the window asks for an enhancement while telemetry is running, naming a trace and a span of its own
- **THEN** the request's span belongs to that trace, as a child of the span the window named
- **AND** the enhancement's span is a child of the request's span, in the same trace

#### Scenario: A trace that cannot be read

- **WHEN** the window asks for an enhancement naming a trace in a form that cannot be read
- **THEN** the enhancement is served as it would be had the window named no trace
- **AND** the request's span roots a new trace

#### Scenario: The named trace stays out of the file

- **WHEN** an enhancement asked for under a named trace records a failure in the file
- **THEN** the record carries the run's identifier
- **AND** carries nothing of the trace the window named

#### Scenario: A photograph described on another thread

- **WHEN** the window asks for photographs to be described while telemetry is running
- **THEN** each description's span belongs to the trace of the window's request

#### Scenario: A request the window withdrew

- **WHEN** the window cancels a run it asked for while telemetry is running
- **THEN** the span of the request that asked for the run says it stopped
- **AND** neither that span nor the cancellation's own span is marked as failed

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off
- **THEN** the collector receives no span for the request that recorded the choice

### Requirement: A graphical application records each failure only it observes, once

A graphical application can fail at things this library never sees: a link the system would not open, a file manager
that would not show a file, a destination it could not claim, a request that arrived before the application was ready
or named a photograph it never opened. Each such failure of a window's request SHALL be recorded at warning severity,
once, naming the request and the reason.

A failure this library already recorded SHALL NOT be recorded again by the graphical application, at any severity,
as the requirement that a failure is recorded once, where it happened, already demands. A stop SHALL NOT be recorded as
a failure.

Every failure a request can return SHALL fall into exactly one of those three cases: recorded by the library, recorded
by the graphical application, or not a failure.

#### Scenario: A link that will not open

- **WHEN** the window asks for a link to be opened and the system refuses
- **THEN** exactly one record at warning severity names the link and the reason

#### Scenario: A photograph that cannot be read for an enhancement

- **WHEN** the window asks for an enhancement of a photograph that can no longer be read
- **THEN** exactly one record at warning severity or above describes that failure, and it is the library's

#### Scenario: A destination that cannot be claimed

- **WHEN** an export's destination cannot be claimed
- **THEN** exactly one record at warning severity names the destination and the reason
- **AND** no record at error severity is written for it

### Requirement: The end of a graphical session is recorded

A graphical session's file starts with a header that says who started it. Its end SHALL be recorded too, at
informational severity, with how long the session lasted. While telemetry is running the record SHALL reach the
collector, so that a session that ended normally can be told apart from one that stopped sending because it crashed.

A second launch asks the application already running to raise its window. When the running application cannot show,
restore or focus that window, it SHALL record why, in the file rather than on a console nobody reads.

#### Scenario: Quitting the application

- **WHEN** the user quits the graphical application while telemetry is running
- **THEN** the file holds a record saying the session ended and how long it lasted
- **AND** the collector receives that record

#### Scenario: A second launch whose window cannot be raised

- **WHEN** a second launch is refused and the running application cannot show or focus its window
- **THEN** the running application's file holds a warning saying so, with the reason

### Requirement: A graphical application's window records the failures only it observes

A graphical application's window runs code this library never sees, and its failures otherwise go only to a
console no user can open. Each failure only the window observes SHALL be recorded in the file, under the graphical
application's identity. That covers:
- an error raised while drawing the window
- an error nothing caught
- a promise rejection nothing handled
- a failure the window catches and continues through

The window's records SHALL be the file's alone. While the window is sending, each of these failures SHALL reach
Grafana once, through the window's own reporting (see `gui-analytics`), and not a second time as a record.

A record SHALL carry what the window said about the failure, and the error's own text. It SHALL also carry the stack
and, for a drawing error, the drawn component path, when the error has them. Each SHALL be written on one line of
the file, however many lines the stack has.

**Severity:**
- An error raised while drawing the window SHALL be recorded at error severity. It is what the window does not
  survive: nothing is left on screen except the way to recover.
- Every other failure the window observes SHALL be recorded at warning severity.

**Once:**
- A failure the window receives in answer to one of its requests SHALL NOT be recorded by the window, at any
  severity, when the graphical application or this library has already recorded it or it was a stop. The requirement
  that a failure is recorded once, where it happened, already demands that.
- A request that failed without ever reaching the graphical application's handling of it SHALL be recorded by the
  window, because nothing else can. This includes a request that could not be delivered.

Recording a failure SHALL NOT change what the window does about it, and SHALL NOT be a unit of work of its own.
Recording is not a request of the window's that is sent to the collector as a span.

#### Scenario: A drawing error

- **WHEN** part of the window throws an error while it is being drawn
- **THEN** one record at error severity in the file names the error and carries the drawn component path

#### Scenario: A rejection nothing handled

- **WHEN** a promise in the window rejects and nothing handles it
- **THEN** one record at warning severity in the file carries the rejection's reason

#### Scenario: A failure the window catches

- **WHEN** copying a failure's reason to the clipboard is refused
- **THEN** one record at warning severity in the file says the copy was refused and why

#### Scenario: A failure the library already recorded

- **WHEN** the window asks for an enhancement, the enhancement fails, and the window shows the user a notice
- **THEN** exactly one record at warning severity or above describes that failure, and it is the library's

#### Scenario: A stop

- **WHEN** the window stops an analysis it asked for, and the analysis answers that it was stopped
- **THEN** the window records nothing about it

#### Scenario: Records while telemetry runs

- **WHEN** the window records a failure while telemetry is running
- **THEN** the file holds the record
- **AND** the collector receives no record of it and no span for the recording itself

### Requirement: The window's records are bounded

A failure in the window can repeat every time something is drawn, and a file that grows with every frame would bury
everything else. The window's records SHALL therefore be bounded:
- **The same failure** SHALL be recorded once for as long as the window is loaded. A reload starts again.
- **The number of records one load of the window writes** SHALL be bounded. Once the bound is reached, one record
  SHALL say that the window's further records are being dropped, and nothing more is written until a reload.
- **Each text a record carries** SHALL be bounded in length, and a text cut short SHALL say so.

A failure to record SHALL NOT itself be recorded. The window SHALL go on as it would have.

#### Scenario: A failure that repeats

- **WHEN** the same failure happens many times while the window is loaded
- **THEN** the file holds one record of it

#### Scenario: A window that fails without end

- **WHEN** the window observes more distinct failures than one load may record
- **THEN** the file holds records up to the bound, and one record saying the rest are dropped

#### Scenario: A very long stack

- **WHEN** a failure's stack is longer than a record may carry
- **THEN** the record carries the start of it and says it was cut short

#### Scenario: A window with nowhere to record

- **WHEN** a failure is observed while its record cannot be delivered
- **THEN** the window goes on as it would have, and does not try to record that the record was lost
