## Purpose

Every application of this project keeps a durable account of what it did, in one file a user can be
asked to attach to a bug report. This capability covers where that file is, what a reader finds in
it, which sources reach it — this library, the front ends, and the inference runtime's own
diagnostics — how it is kept from growing without bound, and what happens when it cannot be written
at all.

## ADDED Requirements

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
carry the application that produced it, spelled exactly as a single-instance refusal spells it —
`gui`, `cli` or `perf`.

This SHALL hold for every record in the file, including the ones produced by the inference runtime
rather than by this project's own code.

#### Scenario: A record is written

- **WHEN** any record is written by any of the three applications
- **THEN** it carries the application's name as a field
- **AND** the spelling is the same one a single-instance refusal uses for that application

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
