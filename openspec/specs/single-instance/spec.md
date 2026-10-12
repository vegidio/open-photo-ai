# single-instance Specification

## Purpose

At most one Open Photo AI process per user may hold the configuration directory at a time, because
the run cache's store, the model and dependency installs and the log file each assume a single
writer. This capability covers when that claim is made and given up, what the second process is told,
and why a process that died without cleaning up does not lock the application out of itself.

## Requirements

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

A refused initialization SHALL report which application holds the claim — the identity that process
declared when it initialized — and the holding process's identifier, so that the user is told what to
close rather than only that something is running.

Where the holder's identity cannot be read, initialization SHALL still be refused, reporting that the
application is already running without naming the holder. The recorded identity SHALL NOT decide
whether a process may proceed: an absent, empty, truncated or unrecognisable record is a refusal that
names no holder, never a grant.

The identity is whatever the holding process declared rather than one of a fixed set of applications.
This project's own binaries declare `gui`, `cli` and `perf`, and a refusal naming one of those SHALL
read exactly as it did before; an application embedding this library declares its own, and a refusal
names that.

#### Scenario: The holder is named

- **WHEN** initialization is refused because the graphical application holds the claim
- **THEN** the error identifies the graphical application and the holding process's identifier

#### Scenario: An embedding application holds the claim

- **WHEN** initialization is refused because a process that declared its own identity holds the claim
- **THEN** the error names that identity and the holding process's identifier

#### Scenario: The holder's identity cannot be read

- **WHEN** initialization is refused and the recorded holder identity is missing, empty or
  unrecognisable
- **THEN** initialization is still refused
- **AND** the error reports that the application is already running without naming a holder

### Requirement: An application declares who it is, or is known by the name it initialized under

Initialization SHALL accept an identity for the process, and SHALL NOT require one. Where none is
declared, the application name the caller initialized under SHALL be used as the identity.

A default had to be chosen and this is the only one that invents nothing. Any fixed alternative
either names the caller as a binary it is not — recording "the command-line application" for a
desktop application that forgot to say otherwise — or introduces a placeholder that tells a user
nothing about what to close. The name the caller has already supplied is in hand, is what every
directory of that installation is resolved from, and names nothing the caller is not. It is not
exempt from the rule below: a name may be a legible directory segment and still not be recordable as
an identity, and where it is not, initialization is refused rather than the name being coped with.

#### Scenario: An application that declares an identity

- **WHEN** a process initializes declaring an identity
- **THEN** that identity is what a refusal naming this process reports
- **AND** it is what each of this process's log records carries

#### Scenario: An application that declares none

- **WHEN** a process initializes without declaring an identity
- **THEN** the application name it initialized under is used as its identity

### Requirement: An identity that would not survive being recorded is refused

The recorded holder is one line naming the identity and the process identifier, read back by taking
the identity as the text before the first space. An identity SHALL therefore be refused where it is
empty, contains whitespace, or contains control characters, and initialization SHALL fail naming what
was rejected and why.

This SHALL hold for the identity a process is known by, whether it declared one or is known by the
name it initialized under. A name is validated as a directory segment, which permits whitespace the
record cannot carry, so the defaulted identity is subject to this rule rather than exempt from it.

Refusing at the point of declaration rather than accepting and coping is what keeps the failure
legible. An identity containing a space is not an error anywhere it is written; it is an error only
when someone else reads it back, and by then the only symptom is that a refused user is told
something is running without being told what — the exact outcome the holder record exists to prevent,
reappearing as a silent degradation rather than as a fault anyone can find.

#### Scenario: An identity containing a space

- **WHEN** a process initializes declaring an identity containing a space
- **THEN** initialization fails, naming the rejected identity and the reason
- **AND** no claim is taken and nothing is installed

#### Scenario: An empty identity

- **WHEN** a process initializes declaring an empty identity
- **THEN** initialization fails, naming the reason

#### Scenario: An identity containing a control character

- **WHEN** a process initializes declaring an identity containing a line break or other control
  character
- **THEN** initialization fails, naming the reason

#### Scenario: A name that cannot be recorded as an identity

- **WHEN** a process initializes declaring no identity, under a name that is a usable directory
  segment but could not be recorded as an identity
- **THEN** initialization fails, naming the rejected name and the reason
- **AND** no claim is taken and nothing is installed

#### Scenario: A recorded identity round-trips

- **WHEN** a process declares an accepted identity, takes the claim, and another process is refused
  by it
- **THEN** the refusal names exactly the identity that was declared

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

### Requirement: A second launch of the graphical application raises the window that already exists

Launching the graphical application while an instance of it is already running for the same user
SHALL NOT open a second window and SHALL NOT report an error. The second launch SHALL end
successfully, and the running instance's window SHALL be made visible, restored if it was minimised,
and given keyboard focus.

Raising the window SHALL NOT change its size or position. A window the user had maximised SHALL
still be maximised afterwards, since the user asked to see the application rather than to resize it.

Anything supplied on the second launch — command-line arguments, the directory it was launched from —
SHALL NOT alter the running application's state. A second launch is a request to see the application,
and nothing else.

#### Scenario: The running window is minimised

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user and that instance's window is minimised
- **THEN** the window is restored and given focus
- **AND** no second window is opened

#### Scenario: The running window is maximised

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user and that instance's window is maximised
- **THEN** the window is given focus and is still maximised

#### Scenario: The second launch is not a failure

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user
- **THEN** the second launch ends successfully, reporting no error to the user and to whatever
  started it

#### Scenario: Arguments on the second launch change nothing

- **WHEN** the graphical application is launched a second time with command-line arguments
- **THEN** the running instance is raised and is otherwise unchanged

### Requirement: A second launch is harmless when the window cannot be raised

Where the running instance has not yet opened its window, a second launch SHALL still end without
opening a window of its own, and the running instance SHALL be unaffected and SHALL continue
starting.

#### Scenario: The running instance is still starting

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user but that instance has not yet opened its window
- **THEN** the second launch ends without opening a window
- **AND** the running instance continues starting normally

### Requirement: A second launch is recognised across versions of the application

Two builds of the graphical application SHALL be treated as the same application regardless of their
version numbers, so that launching one while the other is running raises the running one rather than
starting a second application alongside it.

Two versions running side by side would open the same configuration directory, which is the outcome
the single-instance claim exists to prevent.

#### Scenario: A different version is launched

- **WHEN** one version of the graphical application is launched while a different version of it is
  already running for the same user
- **THEN** the running instance is raised and the launched one ends without opening a window

### Requirement: Raising the window is scoped to the launching user

The behaviour above SHALL be scoped to the user who launched the application, matching the scope of
the single-instance claim. Two users signed into one machine SHALL each run their own graphical
application, and a launch by one SHALL NOT raise a window belonging to the other.

#### Scenario: Two users on one machine

- **WHEN** two users signed into the same machine each launch the graphical application
- **THEN** both run, each with its own window
- **AND** neither user's launch raises the other user's window

### Requirement: An unavailable activation mechanism does not stop the application from starting

Where the facility the platform provides for reaching an already-running instance is unavailable,
unreachable or misconfigured, the graphical application SHALL start normally without it, rather than
failing to start, exiting silently or reporting an error.

Losing the raise costs a user one extra window. Failing to start costs them the application.

#### Scenario: The platform facility is unreachable

- **WHEN** the graphical application is launched on a system where the facility for reaching an
  already-running instance is not running
- **THEN** the application starts normally

#### Scenario: The platform facility is misconfigured

- **WHEN** the graphical application is launched on a system where that facility is configured with a
  value that cannot be used
- **THEN** the application starts normally

### Requirement: A first launch brings its own window to the front

When the graphical application reveals its window for the first time, it SHALL ask for keyboard focus
as well as for the window to be shown.

A window that is shown but left behind whatever the user was looking at is a launch the user has to
go and find. The second-launch raise below already makes this promise for every launch after the
first; the first one is the launch where it is most likely to matter, because there is no running
instance to have raised.

Failing to bring the window to the front SHALL NOT be treated as a failure to launch. What a refusal
costs is one window that opens behind another, not a launch with nothing on screen, and the request
SHALL be made even when showing the window has already failed - a window that is not visible cannot
take focus, so the call costs nothing and the branch that would save it has no reader.

#### Scenario: The window appears

- **WHEN** the graphical application reveals its window
- **THEN** the window is shown and asked to take keyboard focus

#### Scenario: The front refuses

- **WHEN** the window is shown but the platform refuses to bring it to the front
- **THEN** the application goes on running with its window on screen, and records what was refused
