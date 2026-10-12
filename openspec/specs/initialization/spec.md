# initialization Specification

## Purpose

Everything the application has to put on disk before it can run a model: the ONNX Runtime for the
machine it is running on, and the NVIDIA libraries its graphics hardware can actually use - each
fetched from the project's published releases, proven to be the published artifact, installed under
an application-named configuration directory, and recorded so a later version replaces it rather
than merging into it, with progress reported throughout, because a first launch is a
multi-hundred-megabyte download followed by a minute of expansion.

It also covers what has to be true for those files to be usable once they are down: the installed
directories on the dynamic loader's search path, the runtime library loaded and its process-wide
inference environment started before initialization reports success, and a report of which execution
providers this machine can be asked to run on.

## Requirements

### Requirement: The ONNX Runtime is installed on first initialization

Initialization SHALL take an application name and install the pinned ONNX Runtime for the running
platform under that application's configuration directory, in a `runtime` subdirectory of its own.
The directory is created if it does not exist. Initialization SHALL NOT complete successfully unless
the runtime's files are in place.

The application name determines the configuration directory - `~/.config/<name>` on Linux,
`~/Library/Application Support/<name>` on macOS, `%APPDATA%\<name>` on Windows - so the same name
must be passed on every call for the same installation to be found again.

Initialization SHALL claim the configuration directory for this process before it probes the
machine's hardware, transfers anything or creates anything beneath that directory, and SHALL be
refused where another process of the same user already holds that claim - see the `single-instance`
capability. A refused initialization SHALL leave the configuration directory exactly as it found it
and SHALL transfer nothing.

The application name is validated and the configuration directory resolved before the claim is made,
because the claim is recorded within that directory. A name that cannot be a directory segment is
therefore still rejected as a name rather than as a contended claim.

#### Scenario: First launch on a machine with nothing installed

- **WHEN** initialization runs with an application name and no runtime is present
- **THEN** the pinned archive for the running platform is downloaded, expanded into the `runtime`
  subdirectory of that application's configuration directory, and the archive itself is removed
- **AND** initialization reports success

#### Scenario: An application name that cannot be a directory segment

- **WHEN** initialization runs with a name that is empty, `.`, `..`, or contains a path separator
- **THEN** initialization fails with an error naming the rejected value
- **AND** nothing is created, downloaded or removed on disk

#### Scenario: Another process of the same user is already initialized

- **WHEN** initialization runs with an application name and no runtime is present, while another
  process of the same user is already initialized under that name
- **THEN** initialization fails with an "already running" error
- **AND** no hardware is probed, nothing is downloaded, and nothing is created under the
  configuration directory

### Requirement: The machine's graphics hardware decides which GPU libraries are installed

Initialization SHALL probe the machine's graphics adapters and install the NVIDIA libraries the
detected hardware can use, and only those:

- CUDA and cuDNN SHALL be installed when any adapter is an NVIDIA adapter.
- TensorRT SHALL additionally be installed when any NVIDIA adapter is an RTX-branded card, of any
  product line - consumer, workstation or professional.

RTX branding is the criterion because it is what distinguishes the hardware TensorRT can build for:
the pinned TensorRT requires an architecture no older than Turing, and NVIDIA introduced the RTX
brand with Turing, so no card carrying it predates that requirement.

The probe is a statement about the hardware, not about the driver: an NVIDIA card with a missing or
too-old driver still installs the libraries, and the failure surfaces later as a session that cannot
be built on that provider.

A machine with no NVIDIA adapter SHALL download and install nothing beyond the runtime. A probe that
cannot be completed SHALL be treated as reporting no NVIDIA adapter, so a machine whose adapters
cannot be enumerated initializes on the CPU rather than failing.

#### Scenario: A machine with no NVIDIA adapter

- **WHEN** initialization runs on a machine whose adapters are all non-NVIDIA, or which has none
- **THEN** no GPU library is downloaded or installed
- **AND** initialization completes with the runtime installed

#### Scenario: An NVIDIA card that is not RTX-branded

- **WHEN** initialization runs on a machine whose only NVIDIA adapter carries no RTX branding
- **THEN** CUDA and cuDNN are installed
- **AND** TensorRT is not downloaded or installed

#### Scenario: A consumer RTX card

- **WHEN** initialization runs on a machine with a GeForce RTX adapter
- **THEN** CUDA, cuDNN and TensorRT are all installed

#### Scenario: A workstation or professional RTX card

- **WHEN** initialization runs on a machine whose NVIDIA adapter is a professional RTX card rather
  than a GeForce one - an RTX A-series, an RTX Ada Generation, an RTX PRO or a Quadro RTX
- **THEN** CUDA, cuDNN and TensorRT are all installed, exactly as for a GeForce RTX adapter

#### Scenario: The adapters cannot be enumerated

- **WHEN** the graphics-adapter probe fails
- **THEN** initialization proceeds as though no NVIDIA adapter were present
- **AND** initialization completes rather than failing

### Requirement: Each GPU library is installed under the same rules as the runtime

Every GPU library SHALL be installed into a directory of its own beneath a `libs` directory in the
application's configuration directory - `libs/cuda`, `libs/cudnn` and `libs/tensorrt` - and SHALL be
subject to every rule already stated for installing a dependency: it is resolved to one pinned
archive for the running platform, verified against that archive's published SHA-256 before anything
is extracted, expanded only into its own directory, recorded so that a version bump replaces rather
than merges, skipped when the recorded install is already current and complete, and never mistaken
for finished when an install was interrupted.

No GPU library SHALL share a directory with another, or with the runtime, since replacing one must
not delete or keep the files of another.

#### Scenario: A GPU library whose pinned version was bumped

- **WHEN** initialization runs with a pinned CUDA version different from the one recorded as
  installed
- **THEN** every file the previous CUDA version recorded is deleted before the new archive is
  expanded
- **AND** the cuDNN and TensorRT installs are untouched

#### Scenario: A second launch with the GPU libraries already installed

- **WHEN** initialization runs on a machine where every pinned GPU library is recorded as installed
  and all its files are present at their recorded sizes
- **THEN** nothing is downloaded or extracted for any of them

#### Scenario: A GPU archive that does not match its pinned hash

- **WHEN** a GPU library's archive is downloaded and its hash does not equal the pinned value
- **THEN** it is not expanded, and the same retry-once-from-zero rule that governs the runtime
  applies

### Requirement: Only the archive published for the running platform is installed

The application SHALL resolve the runtime to a single pinned archive - release tag, expected SHA-256,
expected size and the name of the shared library it carries - selected by the operating system and
architecture it is running on. A platform for which no archive is published SHALL be reported as an
error naming that platform.

An unknown or unpublished platform MUST NOT fall back to an unverified install: there is no
"no expected hash" state for a release archive.

#### Scenario: A platform the runtime is published for

- **WHEN** initialization runs on a platform present in the pinned table
- **THEN** the archive published for exactly that operating system and architecture is the one
  downloaded and verified

#### Scenario: A platform the runtime is not published for

- **WHEN** initialization runs on a platform with no published archive
- **THEN** initialization fails with an error naming the operating system and architecture
- **AND** no download is attempted

### Requirement: A provider with no archive published for this platform is offered, not attempted

The NVIDIA libraries are published for a subset of the platforms the runtime is published for. When
the hardware probe reports that a provider's hardware is present but no archive for that provider is
published for the running operating system and architecture, initialization SHALL report that
provider as unsupported and SHALL NOT attempt to install it. This SHALL NOT fail initialization.

This is the one place where an unpublished platform is not an error. The runtime is required for the
application to work at all, so a platform it is not published for is a reported failure; a GPU
library is an optimisation, so a platform it is not published for is an optimisation that is not
offered.

#### Scenario: NVIDIA hardware on a platform with no published CUDA archive

- **WHEN** initialization runs on a platform where an NVIDIA adapter is detected but no CUDA archive
  is published
- **THEN** no download is attempted for it
- **AND** initialization completes successfully with CUDA reported as unsupported

### Requirement: A GPU library that cannot be installed fails initialization

Once a GPU library has been selected for installation - its hardware is present and an archive is
published for the platform - a failure to download, verify, expand or record it SHALL fail
initialization with an error naming the dependency, exactly as a failure of the runtime does.

#### Scenario: The TensorRT download fails

- **WHEN** the TensorRT archive cannot be downloaded on a machine whose hardware supports it
- **THEN** initialization fails with an error naming TensorRT
- **AND** the runtime and any GPU library already installed are left in place for the next run

### Requirement: A downloaded archive is verified before it is expanded

The bytes downloaded SHALL be hashed and compared against the SHA-256 pinned for that archive before
anything is extracted from it. The pinned hash covers the archive as published - not any file inside
it - so a single comparison covers every file the archive carries.

An archive whose hash does not match SHALL NOT be expanded, and its bytes SHALL be deleted rather
than left on disk where a later run could resume onto them. A mismatch after a resumed transfer SHALL
be retried once as a complete download from the start, since a stale prefix is the likeliest cause; a
mismatch on a transfer that started from zero SHALL fail immediately.

#### Scenario: The archive matches its pinned hash

- **WHEN** a download completes and its hash equals the pinned value
- **THEN** the archive is expanded into the install directory

#### Scenario: A resumed transfer produces the wrong bytes

- **WHEN** a transfer that resumed from a partial file completes with a hash other than the pinned
  value
- **THEN** the partial bytes are deleted, the artifact is downloaded again from the start, and only
  that second result decides whether initialization succeeds

#### Scenario: A complete download produces the wrong bytes

- **WHEN** a transfer that started from zero completes with a hash other than the pinned value
- **THEN** initialization fails with an error reporting the expected and actual hashes
- **AND** nothing is extracted and the downloaded bytes are removed

### Requirement: An interrupted transfer resumes only onto bytes of the same artifact

A transfer that is interrupted SHALL leave its partial bytes recoverable, and a later run SHALL
continue them only when they are provably a prefix of the artifact now being requested - the same
URL and the same pinned hash. Partial bytes that cannot be shown to belong to the current request
SHALL be discarded and the transfer started from zero.

A file at an artifact's final path SHALL mean the transfer completed. An interrupted, failed or
cancelled transfer MUST NOT leave a truncated file there.

#### Scenario: Resuming the same artifact

- **WHEN** initialization is interrupted mid-download and run again against the same pinned artifact
- **THEN** the transfer continues from the bytes already on disk rather than starting over

#### Scenario: The pinned version moved while a partial was on disk

- **WHEN** initialization is interrupted mid-download, the pinned runtime version is then bumped, and
  initialization runs again
- **THEN** the stale partial bytes are discarded and the new artifact is downloaded from zero
- **AND** the failure is not left for the hash comparison to catch after the transfer

### Requirement: An install that is already current and complete is not repeated

Initialization SHALL record what a dependency installed and SHALL use that record to decide whether
to install again. An install SHALL be skipped when the recorded identity matches the currently pinned
identity - the release tag, the source URL and the expected hash - and every recorded file is still
present at its recorded size.

The steady-state check SHALL NOT re-hash installed files: it verifies presence and size only, because
the contents were verified against the pinned hash when they were installed.

#### Scenario: Second launch with nothing changed

- **WHEN** initialization runs on a machine where the pinned runtime is already recorded as installed
  and all its files are present at their recorded sizes
- **THEN** nothing is downloaded, nothing is extracted, and initialization completes

#### Scenario: A file was deleted from the install directory

- **WHEN** initialization runs and a recorded file is missing or has a different size
- **THEN** the dependency is installed again

#### Scenario: Symbolic links inside the install are not read as corruption

- **WHEN** an installed runtime contains symbolic links between library versions, as the Linux
  archives do
- **THEN** the presence-and-size check inspects the links themselves rather than their targets, and
  reports the install intact

### Requirement: Replacing a version removes what the previous one installed

When the pinned identity changes, initialization SHALL delete the files the previous version recorded
before installing the new one, so two versions' shared libraries never sit side by side for the
loader to choose between. Directories emptied by that deletion SHALL be removed, but the install
directory itself SHALL be kept.

An install directory holding files with no record of what installed them SHALL be emptied before
extraction, since its contents cannot be described and extracting over them would merge two versions.
Partial downloads and their bookkeeping SHALL survive that emptying, so an interrupted large download
can still be resumed.

#### Scenario: The pinned runtime version is bumped

- **WHEN** initialization runs with a pinned version different from the one recorded as installed
- **THEN** every file the previous version recorded is deleted before the new archive is expanded
- **AND** the new record lists only the files the new archive produced

#### Scenario: An install directory with no record

- **WHEN** initialization finds files in the install directory and no record of what installed them
- **THEN** the directory is emptied before extraction, except for partial downloads and their
  bookkeeping

### Requirement: An interrupted install is never mistaken for a finished one

The record of an install SHALL be removed before the first byte of a new install is written and
SHALL be written again only once every file is in place. The record SHALL be written atomically, so
it is only ever observed complete.

A record that is missing, unreadable, malformed, or written in a format this version does not
understand SHALL be treated as if there were no record at all - costing a reinstall rather than
trusting a partial file list.

#### Scenario: The process is killed mid-extraction

- **WHEN** initialization is interrupted after the previous record was removed and before the new one
  was written, and initialization runs again
- **THEN** the dependency is installed again from the beginning rather than reported as present

#### Scenario: A record left by an incompatible version

- **WHEN** initialization finds a record whose format version it does not recognise
- **THEN** the record is ignored and the dependency is installed again

### Requirement: The installed libraries are on the loader's search path

The application SHALL ensure the directories the GPU libraries are installed into are searched by
the dynamic loader, so that the execution providers shipped inside the runtime can load them.

The application SHALL provide one step, run before anything else the program does, that establishes
that search path. On platforms where the search path can be changed by the running process, that
step adds each library directory to it. On platforms where it cannot - where the loader reads its
search path once when the process starts - that step replaces the process image once with one
started under the corrected path.

Every library directory SHALL be created before it is named on a search path, whether or not
anything has been installed into it yet, because a loader that finds a search-path entry missing at
startup skips it for the rest of the process's life - which would leave a library downloaded later
in the same session unfindable.

The application's own library directories SHALL take precedence over any inherited entries, since
they are version-matched to the runtime, and any inherited entries SHALL be preserved rather than
discarded. A directory SHALL NOT appear on the search path more than once.

The process image SHALL be replaced at most once however many times the entry point runs, and a
replacement that fails SHALL NOT stop the program: startup continues under the inherited search
path, with the failure reported.

#### Scenario: Establishing the search path before the loader reads it

- **WHEN** the program establishes its library search path on a platform where the loader reads it
  at process start
- **THEN** the runtime and all three library directories exist and are on that path, ahead of any
  inherited entries
- **AND** the process is running under that path rather than the one it was started with

#### Scenario: The re-started process does not re-start again

- **WHEN** the program that has already replaced its process image reaches the same entry point
  again
- **THEN** it does not replace it a second time and continues startup

#### Scenario: The process image cannot be replaced

- **WHEN** replacing the process image fails
- **THEN** the failure is reported and startup continues
- **AND** initialization still installs everything the hardware supports

### Requirement: Initialization reports what it is about to install, before it installs it

Initialization SHALL accept a plan recipient and SHALL report to it, **exactly once**, what this
machine's initialization consists of: every dependency it will account for, in the order they will be
handled, each carrying the total published size of the files backing it.

The report SHALL be made after the machine's hardware has been probed and before anything is
transferred, expanded or created beneath the install directories, so that a recipient has the whole
list before any of it starts moving.

The list SHALL name **every** dependency this machine needs, including those already installed, not
only those that turn out to need work. A front end drawing a component list is answering "what does
this machine need, and how far along is it" - a list of only the missing ones cannot say how many
there are in total, and would grow or shrink between two launches of the same application on the same
machine.

The size reported for a dependency SHALL be the published size of its files, which is known before
any transfer begins and is therefore reportable for a dependency that has not started and for one
that will never start because it is already installed.

An initialization with no plan recipient SHALL behave identically and report nothing.

#### Scenario: A first launch on a machine with an RTX card

- **WHEN** initialization runs with a plan recipient on a machine whose NVIDIA adapter is
  RTX-branded, with nothing installed
- **THEN** one plan report arrives naming the runtime, CUDA, cuDNN and TensorRT, in the order they
  will be installed
- **AND** each of them carries its published size
- **AND** that report arrives before any progress report for any of them

#### Scenario: A machine with no NVIDIA adapter

- **WHEN** initialization runs with a plan recipient on a machine whose adapters are all non-NVIDIA,
  or which has none
- **THEN** the plan report names the runtime alone

#### Scenario: A launch on which everything is already installed

- **WHEN** initialization runs with a plan recipient on a machine where every pinned dependency is
  recorded as installed and intact
- **THEN** the plan report names every one of them with its published size, exactly as it would on a
  first launch
- **AND** nothing is transferred

#### Scenario: The plan is reported once, not once per dependency

- **WHEN** initialization runs with a plan recipient and installs more than one dependency
- **THEN** exactly one plan report is made for that initialization

#### Scenario: An initialization refused before the hardware is probed

- **WHEN** initialization is refused because another process of the same user already holds the claim
- **THEN** no plan is reported, because no hardware was probed and nothing was planned

#### Scenario: No plan recipient registered

- **WHEN** initialization runs without a plan recipient
- **THEN** the install behaves identically and no plan is reported

### Requirement: Progress is reported across the download and the expansion

Initialization SHALL accept a progress callback and SHALL report through it across the whole
operation: which dependency is being installed, which phase it is in, the bytes moved so far, the
total where one is known, and an overall fraction between 0 and 1.

The expansion SHALL be reported as work rather than as a pause. Progress SHALL advance during the
expansion of a single large file, not only between files, and the fraction SHALL NOT reach 1 until
the expansion has finished. The final report SHALL land on 1.

Reports SHALL be throttled rather than emitted per chunk, and the fraction SHALL NOT decrease across
reports for one dependency.

#### Scenario: A first install of the runtime

- **WHEN** initialization downloads and expands the runtime with a progress callback registered
- **THEN** reports arrive during the transfer carrying the download phase, then during the expansion
  carrying the expansion phase, and the last report carries a fraction of 1

#### Scenario: An archive that is essentially one large file

- **WHEN** an archive whose content is a single large shared library is expanded
- **THEN** progress advances while that one file is being written, rather than jumping from the start
  of the expansion straight to its end

#### Scenario: No callback registered

- **WHEN** initialization runs without a progress callback
- **THEN** the install behaves identically and reports nothing

### Requirement: Every dependency being installed reports its own progress

Progress SHALL be reported per dependency: each dependency installed in one initialization SHALL
name itself in its reports and SHALL run its own fraction from 0 to exactly 1 across its download
and its expansion. A dependency that is skipped because it is already installed SHALL report no
progress - no download phase, no expansion phase and no intermediate fraction. What it reports
instead is one terminal report that it is already in place, which is the subject of its own
requirement.

Progress SHALL NOT be reported as a single fraction across all dependencies, because which
dependencies will be installed is decided by the hardware and the download sizes differ by more than
an order of magnitude, so one combined figure would advance at a rate no front end could present
honestly.

#### Scenario: A first launch on a machine with an RTX card

- **WHEN** initialization installs the runtime, CUDA, cuDNN and TensorRT with a progress callback
  registered
- **THEN** reports arrive naming each of the four in turn
- **AND** the last report for each of them carries a fraction of 1

#### Scenario: One dependency already installed and the rest missing

- **WHEN** initialization runs with the runtime already installed and the GPU libraries missing
- **THEN** no download or expansion report names the runtime
- **AND** reports arrive for each GPU library that is installed

### Requirement: A dependency that is already installed reports that it is

A dependency skipped because its recorded install is current and complete SHALL report **exactly
one** report, through the same recipient that receives progress, naming that dependency and stating
that it is already in place. That report SHALL carry a fraction of 1 and SHALL NOT claim any bytes
were transferred for it.

It SHALL be made at the point in the sequence where that dependency would otherwise have been
installed, so that the order a recipient observes is the install order whether or not any given
dependency had work to do.

This does not weaken the requirement that such an install is not repeated: nothing is downloaded,
nothing is extracted and nothing is re-hashed. The report says that there was nothing to do, which is
not the same as saying nothing about it - a front end showing a component list otherwise cannot tell
a dependency that is finished from one that has not been reached.

An initialization with no progress recipient SHALL behave identically and report nothing.

#### Scenario: A dependency that is already installed and intact

- **WHEN** initialization runs with a progress recipient and a dependency's recorded files are all
  present at their recorded sizes
- **THEN** exactly one report names that dependency and states it is already in place
- **AND** nothing is downloaded and nothing is extracted for it

#### Scenario: Ordering against a dependency that does have work

- **WHEN** initialization runs with the runtime already installed and a GPU library missing
- **THEN** the runtime's already-in-place report arrives before the first progress report for the GPU
  library

#### Scenario: A second launch with everything already installed

- **WHEN** initialization runs with a progress recipient on a machine where every pinned dependency
  is recorded as installed and intact
- **THEN** each of them reports exactly once that it is already in place
- **AND** no report carries a download or an expansion phase

### Requirement: Initialization reports which execution providers this machine supports

Initialization SHALL report, on success, which execution providers the machine can be asked to use:

- CPU SHALL always be reported as supported.
- CoreML SHALL be reported as supported on macOS 12 or newer, and unsupported on any other operating
  system and on older macOS versions, because that is what the model format the provider is
  configured with requires.
- CUDA and TensorRT SHALL be reported as supported when their hardware was detected and their
  libraries were installed, and unsupported otherwise.

The report is what the machine can be asked for, not a promise that a session will build: a provider
reported as supported can still fail at session-build time, and the application falls back to the
CPU when it does.

#### Scenario: A machine with an RTX card on a platform the libraries are published for

- **WHEN** initialization completes on such a machine
- **THEN** the report marks CPU, CUDA and TensorRT supported, and CoreML unsupported

#### Scenario: An Apple Silicon Mac

- **WHEN** initialization completes on a Mac running macOS 12 or newer
- **THEN** the report marks CPU and CoreML supported, and CUDA and TensorRT unsupported

#### Scenario: A Mac older than macOS 12

- **WHEN** initialization completes on a Mac running a macOS version earlier than 12
- **THEN** the report marks CoreML unsupported

### Requirement: The installed runtime is loaded and its inference environment started

Initialization SHALL load the shared library the pinned runtime archive names, from the `runtime`
directory it was installed into, and SHALL start the inference environment every model session is
later built under. Both happen before initialization reports success.

The library that is loaded SHALL be the one this application installed and verified. Initialization
MUST NOT load a runtime found elsewhere on the machine, and MUST NOT obtain one by any means other
than the install described by the requirements above - the pinned archive, proven against its
published hash, is the only runtime the application ever runs.

The environment SHALL be created during initialization rather than deferred to the first time a
model is needed, so that a runtime which cannot be loaded, or an environment which cannot be
started, is a failed initialization rather than a failure in the middle of a user's first
enhancement. Either failure SHALL be reported as an error naming the library path it was attempted
against.

#### Scenario: A successful first launch

- **WHEN** initialization has installed the pinned runtime and starts its environment
- **THEN** the shared library named by the pinned archive is loaded from the application's `runtime`
  directory
- **AND** the inference environment is running by the time initialization reports success

#### Scenario: The runtime library is missing or unreadable

- **WHEN** the shared library named by the pinned archive is not present in the `runtime` directory,
  or cannot be loaded
- **THEN** initialization fails with an error naming the library path
- **AND** no inference environment is left running

#### Scenario: The library is not a runtime this build can use

- **WHEN** the library at that path loads but is not an ONNX Runtime of a version this build
  supports
- **THEN** initialization fails with an error naming the library path and the version found
- **AND** the application does not fall back to any other runtime on the machine

### Requirement: The inference environment is started once and lives as long as the process

The inference environment is process-wide. Initialization SHALL start at most one for the life of
the process, and that environment SHALL stay valid until the process exits. There is no operation
that tears it down while the application is running.

Initialization MAY be called more than once in one process - a front end that restarts its
application layer after an error does exactly that. A repeated initialization SHALL NOT start a
second environment, SHALL NOT fail because one is already running, and SHALL leave the running one
in place and usable. Everything else initialization does still happens: the dependency installs are
re-checked and the provider report is produced as though it were the first call.

A repeated initialization under a **different** application name SHALL install into that name's
configuration directory as it would on a first call, but SHALL continue to use the runtime library
loaded by the first call. The first runtime a process loads is the runtime that process uses.

Where the first initialization **failed** to load the runtime, no later initialization in that
process SHALL report success. The application MUST NOT treat a retry as having loaded a runtime it
did not load. Every later initialization SHALL fail with an error naming the library path the first
attempt was made against and what it reported, so that a front end can tell the user that the
process has to be restarted rather than retried.

#### Scenario: Initialization runs a second time in the same process

- **WHEN** initialization is called again in a process where it has already succeeded
- **THEN** it succeeds
- **AND** no second environment is started and the existing one remains usable

#### Scenario: A second initialization under a different application name

- **WHEN** initialization is called again with a different application name
- **THEN** the dependencies are installed under that name's configuration directory
- **AND** the runtime library loaded by the first call is the one still in use

#### Scenario: Initialization is retried after the runtime failed to load

- **WHEN** initialization has already failed because the runtime library could not be loaded, and it
  is called again in the same process
- **THEN** it fails with an error naming the library path the first attempt used and what that
  attempt reported
- **AND** it does not report success, and no inference environment is started

#### Scenario: Every handle to the initialized application is released

- **WHEN** the last handle returned by initialization is dropped and initialization is then called
  again
- **THEN** it succeeds and the environment is still usable
- **AND** nothing created under the environment is invalidated

### Requirement: Runtime diagnostics are reported at warning level and telemetry is off

The inference runtime produces its own diagnostics. Initialization SHALL set their level to warning,
which is where the node-assignment and execution-provider fallback messages that explain a slow or
unexpectedly-CPU-bound session appear, without the per-node flood that the informational and verbose
levels produce.

Initialization SHALL disable the runtime's telemetry. The application MUST NOT report what its user
is editing - graph and operator metadata, execution provider names, error text or inference counts -
to any third party.

The environment SHALL be identified by the application name initialization was given, so that a
diagnostic can be attributed to the application that produced it.

Those diagnostics SHALL be delivered to the application rather than written to the process's terminal
output. Where the application has installed a log sink they are recorded in its log, under the
`logging` capability's terms; where it has not, they are discarded. Neither outcome changes the level
above: what the runtime reports is decided here, and where it goes is decided by the application.

#### Scenario: The runtime emits diagnostics

- **WHEN** the runtime reports a warning or an error
- **THEN** it is reported
- **AND** informational and verbose runtime messages are not

#### Scenario: Where a diagnostic goes

- **WHEN** the runtime reports a warning or an error while the application has a log sink installed
- **THEN** it is recorded in the application's log
- **AND** it is not written to the process's terminal output

#### Scenario: A diagnostic with no sink installed

- **WHEN** the runtime reports a warning or an error and the application has installed no log sink
- **THEN** it is discarded
- **AND** nothing is written to the process's terminal output

#### Scenario: Telemetry

- **WHEN** initialization starts the environment
- **THEN** the runtime's telemetry is disabled for every session created under it

### Requirement: Extraction cannot write outside the install directory

Expanding a downloaded archive SHALL NOT create or modify any file outside the dependency's install
directory, whatever the archive's entries claim. An entry naming an absolute path, a parent-directory
component, or a path that resolves outside the install directory through a symbolic link SHALL be
refused rather than written.

This holds on every platform, regardless of which platform's archive is being expanded.

#### Scenario: An archive entry that escapes the install directory

- **WHEN** an archive containing an entry that resolves outside the install directory is expanded
- **THEN** nothing is written outside that directory
- **AND** initialization reports the failure rather than completing quietly

### Requirement: Compiled provider artifacts are discarded when the runtime version changes

Initialization SHALL discard everything the execution providers have compiled from model weights,
together with the measurements shared across models, whenever the ONNX Runtime version in force
differs from the one those artifacts were compiled under. What a provider compiles is valid only for
the runtime that built it, and a stale artifact is at best a wasted rebuild and at worst loaded
against a runtime that cannot use it.

The decision SHALL be made against a durable record of the version the artifacts were compiled under,
rather than inferred from the runtime having just been replaced: an application that stopped between
replacing the runtime and discarding the artifacts would otherwise find a matching runtime on its
next start, skip the reinstall, and never discard them.

Where that record already names the version in force, initialization SHALL discard nothing, so this
costs a launch with nothing to do no work beyond the check.

Discarding them SHALL NOT remove any model's weights, and SHALL NOT fail initialization on a machine
that has never compiled anything.

#### Scenario: The pinned runtime version is bumped

- **WHEN** initialization runs and the compiled artifacts were compiled under a different runtime
  version
- **THEN** every compiled artifact and the shared measurements are discarded
- **AND** the version now in force is recorded

#### Scenario: A launch with the same runtime

- **WHEN** initialization runs and the recorded version is the one in force
- **THEN** nothing is discarded

#### Scenario: A machine that has never run a model

- **WHEN** initialization runs on a machine where nothing has ever been compiled
- **THEN** the version in force is recorded
- **AND** initialization completes

#### Scenario: Model weights are not touched

- **WHEN** the compiled artifacts are discarded because the runtime version changed
- **THEN** every installed model's files remain in place
- **AND** no model is re-downloaded on their account
