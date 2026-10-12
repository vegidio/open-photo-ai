## ADDED Requirements

### Requirement: Records are also sent to a collector once the application starts telemetry

The file is the account a user can be asked for. A collector is where records arrive without their being asked. Once
an application has started telemetry, the records that reach the file SHALL also be sent to this project's collector
if the build names one: this library's, the front end's and the inference runtime's. Units of work that were
recorded as spans SHALL be sent as traces, and a record emitted inside one SHALL carry that trace.

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
