## ADDED Requirements

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

### Requirement: Every dependency being installed reports its own progress

Progress SHALL be reported per dependency: each dependency installed in one initialization SHALL
name itself in its reports and SHALL run its own fraction from 0 to exactly 1 across its download
and its expansion. A dependency that is skipped because it is already installed SHALL report
nothing.

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
- **THEN** no report names the runtime
- **AND** reports arrive for each GPU library that is installed
