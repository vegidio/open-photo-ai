## MODIFIED Requirements

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

## ADDED Requirements

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

## REMOVED Requirements

### Requirement: The machine's graphics hardware decides which GPU libraries are installed

**Reason**: The rule it states (CUDA for any NVIDIA adapter, TensorRT for RTX-branded cards, the driver not considered)
is no longer what the application does. Selection already uses the NVIDIA driver's CUDA version and the first CUDA
device's compute capability, because installing CUDA where the driver or device cannot run it costs a large download
and a failed session build per model. This change moves that rule into the shared dependency library.

**Migration**: Replaced by "The NVIDIA driver and device decide which GPU libraries are installed".
