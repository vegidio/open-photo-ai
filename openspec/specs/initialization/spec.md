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

Initialization SHALL take an application name and install the ONNX Runtime release this build names, for the running
platform, into a shared dependency root: the `io.vinicius.ai` directory in the user's configuration directory. Inside
that root the runtime has a folder of its own, `<dependency>/<version>` (for the 1.30.0 runtime, `onnx/1.30.0`), named
after the Hugging Face folder it is published in. The folder is created if it does not exist. Initialization SHALL NOT
complete successfully unless the runtime's files are in place.

The dependency root is shared by every application that installs from the same published folders, so that the same
runtime is installed once per machine rather than once per application. The application name still determines the
application's own configuration directory: the user's configuration directory joined with `<name>`. That directory
holds the application's models, caches and settings, and the same name must be passed on every call for them to be
found again.

Initialization SHALL claim the application's configuration directory for this process before it probes the machine's
hardware, transfers anything or creates anything beneath that directory or the dependency root, and SHALL be refused
where another process of the same user already holds that claim (see the `single-instance` capability). A refused
initialization SHALL leave both directories exactly as it found them and SHALL transfer nothing.

The application name is validated and the configuration directory resolved before the claim is made, because the claim
is recorded within that directory. A name that cannot be a directory segment is therefore still rejected as a name
rather than as a contended claim.

#### Scenario: First launch on a machine with nothing installed

- **WHEN** initialization runs with an application name and no runtime is present in the dependency root
- **THEN** the archive published for the running platform is downloaded, expanded into the runtime's
  `<dependency>/<version>` folder under `io.vinicius.ai` in the user's configuration directory, and the archive itself
  is removed
- **AND** initialization reports success

#### Scenario: Another application already installed the same runtime

- **WHEN** initialization runs and the dependency root already holds an intact install of the runtime release this
  build names, installed by another application
- **THEN** that install is used as it is, and nothing is downloaded for it

#### Scenario: An application name that cannot be a directory segment

- **WHEN** initialization runs with a name that is empty, `.`, `..`, or contains a path separator
- **THEN** initialization fails with an error naming the rejected value
- **AND** nothing is created, downloaded or removed on disk

#### Scenario: Another process of the same user is already initialized

- **WHEN** initialization runs with an application name and no runtime is present, while another process of the same
  user is already initialized under that name
- **THEN** initialization fails with an "already running" error
- **AND** no hardware is probed, nothing is downloaded, and nothing is created under the configuration directory or
  the dependency root

### Requirement: The NVIDIA driver and device decide which GPU libraries are installed

Initialization SHALL probe the machine's NVIDIA driver and CUDA devices and install the NVIDIA libraries the machine
can actually run, and only those:

- CUDA and cuDNN SHALL be installed when the NVIDIA driver supports at least the CUDA version the published CUDA
  release requires (CUDA 13.0 for the 13.3.0 release), and the first CUDA device has at least the compute capability
  it requires (7.5).
- TensorRT SHALL additionally be installed when CUDA is installed, since TensorRT runs on it, and the first CUDA device
  meets TensorRT's own minimum compute capability (7.5).

A machine that does not qualify SHALL install nothing beyond the runtime, and initialization SHALL record why at
informational level: no driver answered, the driver supports too old a CUDA version, the driver reports no device, or
the device's compute capability is too low. Each reason names the versions involved. From outside, "an NVIDIA card but
no CUDA" otherwise looks identical to a failed install.

A probe that cannot be completed SHALL be treated as reporting no usable NVIDIA device, so a machine whose driver
cannot be queried initializes without the NVIDIA libraries rather than failing.

#### Scenario: A machine with no NVIDIA driver

- **WHEN** initialization runs on a machine where no NVIDIA driver answers
- **THEN** no GPU library is downloaded or installed
- **AND** initialization completes with the runtime installed

#### Scenario: A driver too old for the published CUDA

- **WHEN** initialization runs on a machine whose NVIDIA driver supports a CUDA version older than the published CUDA
  release requires
- **THEN** no GPU library is downloaded or installed
- **AND** a record names the driver's CUDA version and the version required

#### Scenario: A device too old for CUDA

- **WHEN** initialization runs on a machine whose first CUDA device has a compute capability below 7.5
- **THEN** no GPU library is downloaded or installed
- **AND** a record names the device, its compute capability and the one required

#### Scenario: A device that qualifies for CUDA and TensorRT

- **WHEN** initialization runs on a machine whose driver and first CUDA device meet both CUDA's and TensorRT's
  minimums
- **THEN** CUDA, cuDNN and TensorRT are all installed

#### Scenario: The probe fails

- **WHEN** the NVIDIA probe cannot be completed
- **THEN** initialization proceeds as though no usable NVIDIA device were present
- **AND** initialization completes rather than failing

### Requirement: Each GPU library is installed under the same rules as the runtime

Every GPU library SHALL be installed into a folder of its own in the shared dependency root, named after the Hugging
Face folder it is published in (`cuda/<version>`, `cudnn/<version>` and `tensorrt/<version>`). Each SHALL be subject
to every rule already stated for installing a dependency: it is resolved to the one archive published for the running
platform, verified against that archive's published SHA-256 before anything is extracted, expanded only into its own
folder, skipped when it is already installed and intact, never mistaken for finished when an install was interrupted,
and replaces the folders of its other versions when a new version is installed.

No GPU library SHALL share a folder with another, or with the runtime, since replacing one must not delete or keep the
files of another.

#### Scenario: A GPU library whose pinned version was bumped

- **WHEN** initialization runs with a CUDA version different from the one installed in the dependency root
- **THEN** the new version is installed into its own folder and the previous version's folder is removed
- **AND** the cuDNN and TensorRT installs are untouched

#### Scenario: A second launch with the GPU libraries already installed

- **WHEN** initialization runs on a machine where every GPU library this build names is installed and all its files
  are present at their recorded sizes
- **THEN** nothing is downloaded or extracted for any of them

#### Scenario: A GPU archive that does not match its pinned hash

- **WHEN** a GPU library's archive is downloaded and its hash does not equal the published value
- **THEN** it is not expanded, and the same retry-once-from-zero rule that governs the runtime applies

### Requirement: Only the archive published for the running platform is installed

Each dependency SHALL be named by the Hugging Face folder that holds its archives, one per platform. The application
SHALL install the one archive in that folder published for the operating system and architecture it is running on,
verified against the SHA-256 the Hugging Face API reports for it. A platform for which the runtime's folder holds no
archive SHALL be reported as an error naming that platform.

An unknown or unpublished platform MUST NOT fall back to an unverified install: there is no "no expected hash" state
for an archive.

#### Scenario: A platform the runtime is published for

- **WHEN** initialization runs on a platform for which the runtime's folder holds an archive
- **THEN** the archive published for exactly that operating system and architecture is the one downloaded and verified

#### Scenario: A platform the runtime is not published for

- **WHEN** initialization runs on a platform for which the runtime's folder holds no archive
- **THEN** initialization fails with an error naming the operating system and architecture
- **AND** no archive download is attempted

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

The bytes downloaded SHALL be hashed and compared against the SHA-256 the Hugging Face API reports for that archive
before anything is extracted from it. The hash covers the archive as published, not any file inside it, so a single
comparison covers every file the archive carries. All of one initialization's requests SHALL be made against the
single repository commit resolved at its start, so an archive re-uploaded during the initialization cannot be compared
against the other upload's hash.

An archive whose hash does not match SHALL NOT be expanded, and its bytes SHALL be deleted rather than left on disk
where a later run could resume onto them. A mismatch SHALL be retried once as a complete download from the start,
since a stale prefix is the likeliest cause. A second mismatch SHALL fail the install.

#### Scenario: The archive matches its pinned hash

- **WHEN** a download completes and its hash equals the published value
- **THEN** the archive is expanded into the dependency's folder

#### Scenario: A resumed transfer produces the wrong bytes

- **WHEN** a transfer that resumed from a partial file completes with a hash other than the published value
- **THEN** the partial bytes are deleted, the archive is downloaded again from the start, and only that second result
  decides whether initialization succeeds

#### Scenario: A complete download produces the wrong bytes

- **WHEN** the download made from the start also completes with a hash other than the published value
- **THEN** initialization fails with an error naming the dependency as damaged
- **AND** nothing is extracted and the downloaded bytes are removed

### Requirement: An interrupted transfer resumes only onto bytes of the same artifact

A transfer that is interrupted SHALL leave its partial bytes recoverable, and a later run SHALL continue them only when
they are provably a prefix of the archive now being requested, identified by its published hash. That holds even when
the repository has moved to a newer commit, which changes the download URL but not the archive. Partial bytes that
cannot be shown to belong to the current archive SHALL be discarded and the transfer started from zero.

A file at an archive's final path SHALL mean the transfer completed. An interrupted, failed or cancelled transfer MUST
NOT leave a truncated file there.

#### Scenario: Resuming the same artifact

- **WHEN** initialization is interrupted mid-download and run again against the same published archive
- **THEN** the transfer continues from the bytes already on disk rather than starting over

#### Scenario: The pinned version moved while a partial was on disk

- **WHEN** initialization is interrupted mid-download, the archive published in that folder is then replaced, and
  initialization runs again
- **THEN** the stale partial bytes are discarded and the new archive is downloaded from zero
- **AND** the failure is not left for the hash comparison to catch after the transfer

### Requirement: An install that is already current and complete is not repeated

Initialization SHALL record what a dependency installed: the hash of the archive it came from, and every file it
produced with its size. It SHALL use that record to decide whether to install again. An install SHALL be skipped when
the record is there and every recorded file is still present at its recorded size.

That decision SHALL be made **locally**. A launch on which every dependency is installed and intact SHALL make no
network request for them, and SHALL NOT wait on the network at all, so a machine that is offline, or whose network is
slow or hanging, starts as quickly as one that is online. A version folder only ever holds that one version, so the
folder already identifies what is installed. Only a dependency that is missing or damaged needs the network.

The steady-state check SHALL NOT re-hash installed files: it verifies presence and size only, because the contents
were verified against the published hash when they were installed.

#### Scenario: Second launch with nothing changed

- **WHEN** initialization runs on a machine where the runtime release this build names is installed and all its files
  are present at their recorded sizes
- **THEN** nothing is downloaded, nothing is extracted, no network request is made for it, and initialization
  completes

#### Scenario: Second launch while offline

- **WHEN** initialization runs with no network connection on a machine where every dependency this build names is
  installed and intact
- **THEN** initialization completes without waiting for any network timeout

#### Scenario: A file was deleted from the install directory

- **WHEN** initialization runs and a recorded file is missing or has a different size
- **THEN** the dependency is installed again

#### Scenario: Symbolic links inside the install are not read as corruption

- **WHEN** an installed runtime contains symbolic links between library versions, as the Linux archives do
- **THEN** the presence-and-size check inspects the links themselves rather than their targets, and reports the install
  intact

### Requirement: Replacing a version removes what the previous one installed

Every version of a dependency SHALL be installed into a folder of its own. When a version is installed, initialization
SHALL remove that dependency's other version folders from the dependency root, so that old releases do not accumulate
gigabytes on disk. A version folder that another process holds locked while installing into it SHALL be left in place.

The previous version's files are never merged with the new one's, since they live in different folders. A version
folder holding files with no record of what installed them SHALL be emptied before extraction, since its contents
cannot be described and extracting over them would merge two installs. Partial downloads and their bookkeeping SHALL
survive that emptying, so an interrupted large download can still be resumed.

#### Scenario: The pinned runtime version is bumped

- **WHEN** initialization runs with a runtime version different from the one installed in the dependency root
- **THEN** the new version is installed into its own folder
- **AND** the previous version's folder is removed once the new one is in place

#### Scenario: Another application is installing the old version

- **WHEN** a new version is installed while another process holds the old version's folder locked
- **THEN** the old version's folder is left in place

#### Scenario: An install directory with no record

- **WHEN** initialization finds files in a version folder and no record of what installed them
- **THEN** the folder is emptied before extraction, except for partial downloads and their bookkeeping

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
- CoreML SHALL be reported as supported on macOS and unsupported on any other operating system. Every macOS the
  runtime can load on (macOS 14 and newer for the 1.30.0 runtime) supports the model format the provider is configured
  with, so no separate OS-version probe is made.
- CUDA and TensorRT SHALL be reported as supported when the machine qualified for them and their libraries were
  installed, and unsupported otherwise.
- WebGPU SHALL be reported as supported when the WebGPU plugin shipped with the runtime registered and offered at
  least one device, and unsupported otherwise.

The report is what the machine can be asked for, not a promise that a session will build: a provider reported as
supported can still fail at session-build time, and the application falls back when it does.

#### Scenario: A machine with an RTX card on a platform the libraries are published for

- **WHEN** initialization completes on a machine whose NVIDIA driver and first CUDA device meet CUDA's and TensorRT's
  minimums, as every RTX card does with a current driver, on a platform the libraries are published for
- **THEN** the report marks CPU, CUDA and TensorRT supported, and CoreML unsupported

#### Scenario: An Apple Silicon Mac

- **WHEN** initialization completes on a Mac
- **THEN** the report marks CPU and CoreML supported, and CUDA and TensorRT unsupported
- **AND** WebGPU is marked supported if its plugin registered and offered a device

#### Scenario: A Mac older than macOS 12

- **WHEN** initialization runs on a Mac older than the oldest macOS the runtime can load on, which includes every Mac
  older than macOS 14
- **THEN** initialization fails because the runtime cannot be loaded, as it does today, and no provider report is made

### Requirement: The installed runtime is loaded and its inference environment started

Initialization SHALL load the runtime shared library from the runtime's version folder in the dependency root, and
SHALL start the inference environment every model session is later built under. Both happen before initialization
reports success.

The library that is loaded SHALL be the one installed and verified under the rules above. Initialization MUST NOT load
a runtime found elsewhere on the machine, and MUST NOT obtain one by any means other than that install. The archive
published for this build's runtime version, proven against its published hash, is the only runtime the application
ever runs.

The environment SHALL be created during initialization rather than deferred to the first time a model is needed, so
that a runtime which cannot be loaded, or an environment which cannot be started, is a failed initialization rather
than a failure in the middle of a user's first enhancement. Either failure SHALL be reported as an error naming the
library path it was attempted against.

#### Scenario: A successful first launch

- **WHEN** initialization has installed the runtime and starts its environment
- **THEN** the runtime shared library is loaded from the runtime's version folder in the dependency root
- **AND** the inference environment is running by the time initialization reports success

#### Scenario: The runtime library is missing or unreadable

- **WHEN** the runtime shared library is not present in the runtime's version folder, or cannot be loaded
- **THEN** initialization fails with an error naming the library path
- **AND** no inference environment is left running

#### Scenario: The library is not a runtime this build can use

- **WHEN** the library at that path loads but is not an ONNX Runtime of a version this build supports
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

The same discard SHALL happen when the layout those artifacts were written in differs from the
layout this build writes. Artifacts left in a layout nothing reads any more are never reused and
never removed by anything else, so they would stay on disk indefinitely.

The decision SHALL be made against a durable record of the runtime version and the layout the
artifacts were compiled under, rather than inferred from the runtime having just been replaced: an
application that stopped between replacing the runtime and discarding the artifacts would otherwise
find a matching runtime on its next start, skip the reinstall, and never discard them. A record that
names no layout SHALL be read as a different layout.

Where that record already names the version and the layout in force, initialization SHALL discard
nothing, so this costs a launch with nothing to do no work beyond the check.

Discarding them SHALL NOT remove any model's weights, and SHALL NOT fail initialization on a machine
that has never compiled anything.

#### Scenario: The pinned runtime version is bumped

- **WHEN** initialization runs and the compiled artifacts were compiled under a different runtime
  version
- **THEN** every compiled artifact and the shared measurements are discarded
- **AND** the version and layout now in force are recorded

#### Scenario: The cache layout changed

- **WHEN** initialization runs with the same runtime version, and the record names a different
  layout or no layout at all
- **THEN** every compiled artifact and the shared measurements are discarded
- **AND** the version and layout now in force are recorded

#### Scenario: A launch with the same runtime

- **WHEN** initialization runs and the record names the version and the layout in force
- **THEN** nothing is discarded

#### Scenario: A machine that has never run a model

- **WHEN** initialization runs on a machine where nothing has ever been compiled
- **THEN** the version and layout in force are recorded
- **AND** initialization completes

#### Scenario: Model weights are not touched

- **WHEN** the compiled artifacts are discarded because the runtime version or the layout changed
- **THEN** every installed model's files remain in place
- **AND** no model is re-downloaded on their account

### Requirement: Installs into the shared dependency root are serialised across processes

Because the dependency root is shared between applications, two processes SHALL NOT install into the same dependency
version folder at the same time. An install SHALL hold an exclusive lock on the folder it installs into for the whole
of its download, verification and expansion. A second process that needs the same folder SHALL wait for that lock and
then re-check the folder, using the install the first process completed rather than repeating it.

A lock left by a process that has exited SHALL NOT block later installs.

#### Scenario: Two applications start on a machine with nothing installed

- **WHEN** two applications that install the same runtime release initialize at the same time
- **THEN** the runtime is downloaded and expanded once
- **AND** both applications load that one install

#### Scenario: A process that held the lock crashed

- **WHEN** a process exits while holding the lock on a version folder, and initialization runs again
- **THEN** the install proceeds rather than waiting forever

### Requirement: The install folders of earlier releases are removed

Releases before this one installed the runtime into `runtime` and the GPU libraries into `libs/cuda`, `libs/cudnn` and
`libs/tensorrt`, all inside the application's configuration directory. Initialization SHALL delete those folders, if
they exist, so that their gigabytes do not stay on disk unused.

The deletion is best effort: an absent folder is the ordinary case, and one that cannot be removed SHALL be recorded
and SHALL NOT fail initialization. Nothing else in the application's configuration directory SHALL be touched by it.

#### Scenario: The first launch after updating

- **WHEN** initialization runs on a machine where the previous release installed the runtime and GPU libraries into the
  application's configuration directory
- **THEN** the `runtime` and `libs` folders there are deleted
- **AND** the models, caches and settings in that directory are left in place

#### Scenario: A folder that cannot be deleted

- **WHEN** one of the old folders cannot be removed
- **THEN** the failure is recorded and initialization continues
