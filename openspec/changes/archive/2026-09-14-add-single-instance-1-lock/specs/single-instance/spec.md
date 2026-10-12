## Purpose

At most one Open Photo AI process per user may hold the configuration directory at a time, because
the image cache's store, the model and dependency installs and the log file each assume a single
writer. This capability covers when that claim is made and given up, what the second process is told,
and why a process that died without cleaning up does not lock the application out of itself.

## ADDED Requirements

### Requirement: At most one process per user may be initialized

The application SHALL permit at most one initialized process per user. A process attempting to
initialize while another process of the same user holds the claim SHALL be refused with an error
distinguishable from every other initialization failure.

The claim is scoped to the configuration directory, which each platform resolves per user. Two users
signed into one machine resolve two directories and contend for nothing, so neither is refused
because of the other. Two applications initialized under different names likewise resolve two
directories and both succeed.

The claim is made by the library, not by each application, so every binary that initializes is
subject to it whether or not it asked to be.

#### Scenario: A second process is refused

- **WHEN** a process initializes under an application name while another process of the same user is
  already initialized under that name
- **THEN** initialization fails with an "already running" error, distinguishable from an error about
  the disk, the network or the hardware
- **AND** the first process is unaffected and continues running

#### Scenario: Two users on one machine

- **WHEN** two users signed into the same machine each initialize under the same application name
- **THEN** both succeed, because each resolves its own configuration directory

#### Scenario: Two application names in one process or on one machine

- **WHEN** initialization runs under one application name while another is initialized under a
  different name
- **THEN** both succeed, because the two names resolve two configuration directories that share no
  cache, no models and no log

### Requirement: The refusal names the process holding the claim

A refused initialization SHALL report which application holds the claim — one of the graphical
application, the command-line application or the benchmark — and the holding process's identifier,
so that the user is told what to close rather than only that something is running.

Where the holder's identity cannot be read, initialization SHALL still be refused, reporting that the
application is already running without naming the holder. The recorded identity SHALL NOT decide
whether a process may proceed: an absent, empty, truncated or unrecognisable record is a refusal that
names no holder, never a grant.

#### Scenario: The holder is named

- **WHEN** initialization is refused because the graphical application holds the claim
- **THEN** the error identifies the graphical application and the holding process's identifier

#### Scenario: The holder's identity cannot be read

- **WHEN** initialization is refused and the recorded holder identity is missing, empty or
  unrecognisable
- **THEN** initialization is still refused
- **AND** the error reports that the application is already running without naming a holder

### Requirement: The claim is released when the holding process ends

The claim SHALL be released when the process holding it ends, by any means — a clean shutdown, an
unhandled failure, an immediate termination that runs no cleanup, or the loss of the machine's power.
A subsequent initialization SHALL succeed without the user having to remove a file or override a
check.

There SHALL be no supported way to override or force the claim. A wedged process that has not
released it is holding it correctly, and the reported process identifier is what makes that
actionable.

#### Scenario: A process that terminated without cleanup

- **WHEN** a process holding the claim is terminated immediately and runs no cleanup, and another
  process then initializes under the same name
- **THEN** initialization succeeds

#### Scenario: Ordinary shutdown

- **WHEN** a process holding the claim shuts down normally, and another process then initializes
  under the same name
- **THEN** initialization succeeds

### Requirement: The claim lasts as long as the initialized application

The claim SHALL be held from the moment initialization succeeds until the initialized application is
released, and SHALL NOT be released while any part of the process still holds it. An application
handle that has been shared or duplicated within the process keeps the claim until the last of those
is released.

#### Scenario: A shared application handle

- **WHEN** an initialized application is duplicated within a process and one of the duplicates is
  released
- **THEN** the claim is still held

#### Scenario: The last handle is released

- **WHEN** the last handle to an initialized application is released
- **THEN** the claim is released, and a later initialization in the same process succeeds

### Requirement: Initializing twice in one process does not refuse itself

Initializing more than once in one process under the same application name SHALL succeed, and the
second initialization SHALL NOT be refused by the claim the first one holds. The claim is released
once the last of those initializations is released.

This preserves the existing behaviour of an application layer being restarted after an error within
a running process, which is a supported state rather than a second instance.

#### Scenario: An application layer restarted in place

- **WHEN** a process that is already initialized under an application name initializes again under
  that same name
- **THEN** the second initialization succeeds
- **AND** the claim is still held once the first initialization is released

### Requirement: Work that touches no shared state is not subject to the claim

Reading what the application publishes about itself SHALL NOT require the claim and SHALL NOT be
refused while another process holds it. This covers reporting the application's version and listing
the models the catalogue publishes, neither of which reads or writes the configuration directory.

#### Scenario: Listing models while another process is running

- **WHEN** the models the catalogue publishes are listed while another process holds the claim
- **THEN** the list is produced

#### Scenario: Reporting the version while another process is running

- **WHEN** the application's version is reported while another process holds the claim
- **THEN** the version is produced
