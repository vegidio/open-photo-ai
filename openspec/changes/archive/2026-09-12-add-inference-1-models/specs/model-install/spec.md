## Purpose

Getting a named model's files onto disk and proving they are the published ones: resolving a model
artifact to the files behind it, obtaining the manifest those files are verified against and deciding
what happens when it cannot be obtained, installing a bare graph that has nothing to expand, keeping
each model's files separable from every other model's, and reporting progress across the transfer -
because a model is tens of megabytes at least and several gigabytes at worst, and it is loaded into
the process once it is down.

## ADDED Requirements

### Requirement: A named model artifact installs to disk

The application SHALL install the files backing a named model artifact under the application's
configuration directory, creating what does not exist. An install SHALL NOT report success unless
every file backing that artifact is in place at its published size.

#### Scenario: A model that is not yet installed

- **WHEN** an install runs for an artifact whose files are not on disk
- **THEN** every published file backing that artifact is downloaded and placed under the
  application's configuration directory
- **AND** the install reports success

#### Scenario: An artifact the published listing does not name

- **WHEN** an install runs for an artifact with no entry in any available manifest
- **THEN** the install fails with an error naming that artifact
- **AND** no transfer is attempted

### Requirement: A model is never installed without a published hash

The application SHALL verify every downloaded model file against the SHA-256 published for it, and
SHALL refuse to install a file for which no published hash can be obtained. There SHALL be no path -
including an unreachable manifest, an empty manifest, or an entry that carries no hash - by which a
model file is placed on disk or loaded without having been checked.

This is a deliberate divergence from the reference implementation, which downloads an unknown model
unverified and logs a warning.

#### Scenario: A file whose bytes do not match the published hash

- **WHEN** a model file is downloaded and its hash does not match the published one
- **THEN** the install fails with an error distinguishing a hash mismatch from a transfer failure
- **AND** the mismatched file is not left where an install would be taken for complete

#### Scenario: No hash can be obtained for the artifact

- **WHEN** the published listing cannot be reached, no cached copy is available, and the artifact is
  absent from the manifest built into the application
- **THEN** the install fails with an error naming the artifact and saying no published hash is known
- **AND** no transfer is attempted

#### Scenario: A listing entry that carries no hash

- **WHEN** the published listing names a file but publishes no hash for it
- **THEN** that entry is treated as though the file were absent from the listing
- **AND** resolution continues to the remaining sources rather than installing unverified

### Requirement: The hash is resolved from the first available of four sources

The application SHALL obtain a model's published size and hash from, in order: the project's live
published listing, the copy cached from the last successful read of it, and a manifest built into the
application. Where none of the three names the artifact, the install SHALL be refused.

The manifest built into the application SHALL cover every model published at the time the application
was built, so that a machine which cannot reach the listing can still install and verify those
models.

#### Scenario: The live listing is reachable

- **WHEN** an install runs and the published listing can be read
- **THEN** the sizes and hashes from that listing are the ones used
- **AND** the listing is cached so a later run can use it when the listing cannot be read

#### Scenario: The live listing cannot be read but was read before

- **WHEN** an install runs, the published listing cannot be read, and a cached copy exists
- **THEN** the cached copy's sizes and hashes are used
- **AND** the install proceeds and reports success

#### Scenario: A first run with no network at all

- **WHEN** an install runs for a model published at build time, the listing cannot be read, and no
  cached copy exists
- **THEN** the hash built into the application is used
- **AND** the install proceeds and is verified against it

### Requirement: The whole published listing is read

The application SHALL read the published listing to exhaustion rather than reading a bounded prefix
of it. The number of published models SHALL NOT be a fixed limit anywhere in the application.

A listing read only in part reports a published model as having no known hash, which under the
requirements above refuses an install that should have succeeded.

#### Scenario: A listing longer than one response

- **WHEN** the published listing spans more than a single response
- **THEN** every entry across every response is available for resolution
- **AND** an artifact named only in a later response resolves as readily as one named first

### Requirement: Every file backing a model is installed and verified

A model artifact MAY be published as more than one file, a graph too large for a single file keeping
its weights beside it. The application SHALL install and verify every published file backing the
artifact, and SHALL determine that set from the published listing rather than by guessing file names
on disk.

An install SHALL NOT report success with any of those files missing.

#### Scenario: A model published as a graph and a separate weights file

- **WHEN** an install runs for an artifact the listing names more than one file for
- **THEN** every one of those files is downloaded and verified against its own published hash
- **AND** the install reports success only once all of them are in place

#### Scenario: A file backing a model that sorts near another artifact's name

- **WHEN** the listing names files for two artifacts whose names share a leading portion
- **THEN** each artifact installs only the files published for it
- **AND** neither artifact's file set includes a file belonging to the other

### Requirement: Installing or replacing one model leaves other models untouched

The application SHALL keep each model's files separable from every other model's. Installing a model,
or replacing one version of a model with another, SHALL NOT remove or overwrite any file belonging to
a different model.

Replacing a version SHALL remove what the previous version of *that* model installed, rather than
merging two versions' files together.

#### Scenario: Installing a second model beside a first

- **WHEN** a model is installed while another model is already installed
- **THEN** the already-installed model's files remain in place and unmodified
- **AND** both models are usable afterwards

#### Scenario: Two models that are both part of one operation

- **WHEN** an operation requires two model artifacts and both are installed in turn
- **THEN** both artifacts' files are present once the second install completes

#### Scenario: Replacing a model whose published files have changed

- **WHEN** a model is installed whose published file set differs from the one previously recorded for
  it
- **THEN** the files the previous version installed are removed before the new ones are written
- **AND** no file from the previous version survives alongside the new ones

### Requirement: An install that is already current is not repeated

Where a model's recorded install matches what is published and every recorded file is still present
at its recorded size, the application SHALL treat the model as installed: nothing is transferred and
nothing is reported.

#### Scenario: A model that is already installed and intact

- **WHEN** an install runs for a model whose recorded files are all present at their recorded sizes
- **THEN** no transfer is attempted and no progress is reported
- **AND** the install reports success

#### Scenario: A model whose files were removed after installation

- **WHEN** an install runs for a model whose recorded files are missing or the wrong size
- **THEN** the model is installed again

### Requirement: An interrupted model install is never mistaken for a finished one

The record that a model is installed SHALL be written only once every file backing it is in place. An
install interrupted at any point SHALL be detected as incomplete on the next attempt, and SHALL be
completed or redone rather than treated as current.

#### Scenario: The application stops partway through an install

- **WHEN** an install is interrupted after some files have been written
- **AND** an install runs again for the same model
- **THEN** the model is not reported as already installed
- **AND** the install runs to completion

### Requirement: A model install reports progress

The application SHALL report progress while a model is being installed, naming the model being
installed and carrying how far the install has got as a fraction between 0 and 1 that never
decreases. The fraction SHALL reach 1 only once the install has finished, and SHALL be reported as
reaching it rather than being left wherever the last update fell.

An install with no progress recipient SHALL behave identically and report nothing.

#### Scenario: A model being downloaded

- **WHEN** a model install transfers bytes
- **THEN** progress reports name the model and carry a non-decreasing fraction
- **AND** the final report for that install carries a fraction of exactly 1

#### Scenario: A transfer that resumes onto an existing partial file

- **WHEN** an install resumes a transfer that had already moved some bytes
- **THEN** the reported fraction does not go backwards from what was previously reported

#### Scenario: An install with no progress recipient

- **WHEN** an install runs and no progress recipient was supplied
- **THEN** the install behaves as it otherwise would and reports nothing

### Requirement: Obtaining the published listing does not delay startup

The application SHALL NOT read the published model listing as part of initialization. The listing
SHALL be read no earlier than the first model install that needs it, and once read SHALL serve every
later install in the same run without being read again.

Initialization's behaviour, including which failures it reports, SHALL be unchanged by the presence
of this capability.

#### Scenario: An application that initializes and never installs a model

- **WHEN** initialization completes and no model install is ever requested
- **THEN** the published listing is never read

#### Scenario: Several installs in one run

- **WHEN** more than one model is installed in a single run
- **THEN** the published listing is read at most once
- **AND** every install after the first uses what was already read
