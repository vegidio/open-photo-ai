## Purpose

Everything the application has to put on disk before it can run a model: the ONNX Runtime for the
machine it is running on, fetched from the project's published releases, proven to be the published
artifact, installed under an application-named configuration directory, and recorded so a later
version replaces it rather than merging into it - with progress reported throughout, because a first
launch is a multi-hundred-megabyte download followed by a minute of expansion.

## ADDED Requirements

### Requirement: The ONNX Runtime is installed on first initialization

Initialization SHALL take an application name and install the pinned ONNX Runtime for the running
platform under that application's configuration directory, in a `runtime` subdirectory of its own.
The directory is created if it does not exist. Initialization SHALL NOT complete successfully unless
the runtime's files are in place.

The application name determines the configuration directory - `~/.config/<name>` on Linux,
`~/Library/Application Support/<name>` on macOS, `%APPDATA%\<name>` on Windows - so the same name
must be passed on every call for the same installation to be found again.

#### Scenario: First launch on a machine with nothing installed

- **WHEN** initialization runs with an application name and no runtime is present
- **THEN** the pinned archive for the running platform is downloaded, expanded into the `runtime`
  subdirectory of that application's configuration directory, and the archive itself is removed
- **AND** initialization reports success

#### Scenario: An application name that cannot be a directory segment

- **WHEN** initialization runs with a name that is empty, `.`, `..`, or contains a path separator
- **THEN** initialization fails with an error naming the rejected value
- **AND** nothing is created, downloaded or removed on disk

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
