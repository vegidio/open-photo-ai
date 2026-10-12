# model-install Specification

## Purpose

Getting a named model's files onto disk and proving they are the published ones: resolving a model
artifact to the files behind it, obtaining the manifest those files are verified against and deciding
what happens when it cannot be obtained, installing a bare graph that has nothing to expand, keeping
each model's files separable from every other model's, and reporting progress across the transfer -
because a model is tens of megabytes at least and several gigabytes at worst, and it is loaded into
the process once it is down.

## Requirements

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
including an unreachable manifest, an empty manifest, an entry that carries no hash, or a declaration
of trust in the files already on disk - by which a model file the application itself obtained is
placed on disk without having been checked.

This is a deliberate divergence from the reference implementation, which downloads an unknown model
unverified and logs a warning. It is also a divergence from the reference's debug override, which
suppresses the check on a file it has just downloaded as well as on one already present; here that
override reaches only the second, and the requirement below is its whole contract.

The one path by which an unchecked model file may be **loaded** is a file the operator placed on disk
themselves, in a process that declared at initialization that it trusts such files. That is a
property of what was already there rather than of anything the application fetched, and nothing about
it relaxes the paragraph above.

#### Scenario: A file whose bytes do not match the published hash

- **WHEN** a model file is downloaded and its hash does not match the published one
- **THEN** the install fails with an error distinguishing a hash mismatch from a transfer failure
- **AND** the mismatched file is not left where an install would be taken for complete

#### Scenario: A file whose bytes do not match, in a process that trusts local files

- **WHEN** a model file is downloaded and its hash does not match the published one, in a process
  that declared it trusts the model files on disk
- **THEN** the install fails exactly as it does in any other process
- **AND** the declaration of trust changes nothing about bytes the application obtained itself

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

### Requirement: An install can be stopped, and what it transferred is kept

An install SHALL accept being told it is no longer wanted, and SHALL stop the transfer it is running
rather than carrying it to completion: a model is gigabytes, and the whole value of a stop that a
user can see is that it stops the bytes.

A stopped install SHALL be reported distinguishably from one that failed - nothing is broken, and the
caller asked for it - and SHALL leave the bytes it had already moved on disk, so the next install of
the same model resumes onto them under the rules already stated for a resumed transfer.

The bookkeeping either side of the transfer is not interruptible, and does not need to be: it is
milliseconds, and an install stopped between them is detected as incomplete on the next attempt like
any other interruption.

#### Scenario: An install stopped while it is transferring

- **WHEN** an install in the middle of a transfer is told it is no longer wanted
- **THEN** the transfer stops rather than running to completion
- **AND** the install reports that it stopped rather than that it failed
- **AND** nothing is expanded or recorded as installed

#### Scenario: An install that follows a stopped one

- **WHEN** an install runs for a model whose previous install was stopped mid-transfer
- **THEN** it resumes onto what that transfer had already moved rather than starting from nothing

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

### Requirement: Replacing a model's weights invalidates what was compiled from them

Where a model's files are replaced, the application SHALL also remove whatever an execution provider
compiled from the previous ones. At best a stale compiled artifact is wasted disk; at worst it is
reused against weights it was never built for, which produces an image nothing reports as wrong.

The removal SHALL be part of the same replacement that removes the previous files, rather than
something a later step is trusted to do, so that an interruption cannot leave new weights beside an
old compiled artifact.

An install that is already current, and so transfers nothing, SHALL leave the compiled artifacts
alone - they were compiled from exactly those weights, and discarding them would cost a rebuild of
minutes for no reason.

Installing or replacing one model SHALL NOT remove what was compiled from any other model, and SHALL
NOT remove the timing measurements shared across models.

#### Scenario: A model whose published files have changed

- **WHEN** a model is installed whose published file set differs from the one previously recorded
- **THEN** whatever a provider compiled from the previous weights is removed
- **AND** the new files are installed

#### Scenario: A model that is already installed and intact

- **WHEN** an install runs for a model whose recorded files are all present at their recorded sizes
- **THEN** whatever a provider compiled from those weights is left in place

#### Scenario: Replacing one model beside another

- **WHEN** one model is replaced while another model's compiled artifacts are present
- **THEN** the other model's compiled artifacts remain in place
- **AND** the measurements shared across models remain in place

### Requirement: A process may declare that it trusts the model files already on disk

The application SHALL let the process declare, when it initializes and only then, that a model whose
files are already on disk is used as they are rather than checked against what is published.

It exists for one workflow: a model that has been re-exported and not yet published cannot otherwise
be run at all, because the file under test does not match the published hash and is replaced by the
published weights before anything can measure it. That is the case for benchmarking a change to a
model, and it is the case for debugging one.

The declaration SHALL be made at initialization and SHALL hold for the life of that application
handle. There SHALL be no way to turn it on afterwards — not per run, not through a setting, not
through anything a user interface can reach. A process that did not ask for it when it started
cannot arrive at it later, and a caller reading a run's options cannot be misled about which rule was
in force.

The declaration SHALL apply to model files only. The ONNX Runtime and the GPU libraries SHALL be
verified against their pinned hashes in every process regardless of it: those are downloaded
archives of native code, and no debugging workflow requires hand-placing one.

#### Scenario: A process that declares ordinary handling

- **WHEN** an application initializes without declaring trust in local model files
- **THEN** every model is checked against what is published, exactly as before

#### Scenario: A process that declares trust

- **WHEN** an application initializes declaring that it trusts the model files on disk
- **THEN** that rule is in force for every model install that handle performs

#### Scenario: The runtime and GPU libraries under a declaration of trust

- **WHEN** an application that declared trust in local model files installs the runtime or a GPU
  library
- **THEN** the archive is verified against its pinned hash and size as it is in any other process

### Requirement: Trusted local files are used only when the model is complete on disk

Where a process has declared trust in local model files, the application SHALL use the files on disk
without checking them **only** where every file backing that model is already present. Where any of
them is missing, the model SHALL be installed and verified under the ordinary rules.

The declaration says "what is here is what I want measured". It does not say "fetch me whatever is
missing and ask no questions": a partially present model is not something anyone placed by hand, and
completing it from the published sources is both what the operator wants and the only safe reading.

Where the files are used as they are, nothing SHALL be transferred for that model and no progress
SHALL be reported for it.

#### Scenario: Every file already present

- **WHEN** a model all of whose files are on disk is acquired in a process that declared trust
- **THEN** the files are used as they are, whatever their contents
- **AND** no transfer is attempted, even where the published hash differs from what is on disk

#### Scenario: A file missing

- **WHEN** a model one of whose files is absent is acquired in a process that declared trust
- **THEN** the model is installed from the published sources and verified against the published
  hashes

#### Scenario: A model never installed at all

- **WHEN** a model with nothing on disk is acquired in a process that declared trust
- **THEN** it is downloaded and verified exactly as it would be in any other process

### Requirement: A model used on trust is identified by the files themselves

Where the files on disk are used without being checked, there is no published hash to record as the
model's identity. The application SHALL instead derive that identity from the files as they sit on
disk, such that editing or replacing one of them moves it and leaving them untouched does not.

Two things depend on this, and they pull in opposite directions. Whatever an execution provider
compiled from a model's weights SHALL be discarded when a trusted file changes — a compiled artifact
reused against weights it was never built for produces an image nothing reports as wrong, and this is
the workflow in which weights change most often. Equally, a launch that changed nothing SHALL discard
nothing: re-exporting a model is an iteration loop, and rebuilding a provider's compiled artifact on
every launch costs minutes each time and would make the declaration unusable for the very work it
exists for.

#### Scenario: A trusted model whose files have not changed

- **WHEN** a model used on trust is acquired again, with its files untouched since
- **THEN** nothing is transferred and nothing is reinstalled
- **AND** whatever an execution provider compiled from those weights is left in place

#### Scenario: A trusted model whose file has been replaced

- **WHEN** a model used on trust is acquired again after one of its files has been replaced
- **THEN** whatever an execution provider compiled from the previous weights is removed
- **AND** the replacement file is used as it is, without being checked

### Requirement: A model used on trust is never mistaken for a verified install

The record left behind by a model used on trust SHALL NOT assert that any published file is
installed. A later run of a process that did not declare trust SHALL install that model from the
published sources and verify it, rather than accepting files that were never checked.

Without this, one debugging session would silently downgrade every later session on that machine: the
unverified weights would be recorded as though they were the published ones, and every subsequent
launch would read the record, find it current, and run the unpublished model believing it was the
published one.

Each time a model is used on trust, the application SHALL record that it happened, at a verbosity a
user's ordinary log contains, naming the model. An unverified model is the first thing to suspect in
a report of wrong output, and it must not be necessary to reproduce the problem with the logging
turned up to find out that it was in force.

#### Scenario: A later run without the declaration

- **WHEN** a model has been used on trust, and is then acquired by a process that did not declare
  trust
- **THEN** the model is installed from the published sources and verified
- **AND** the files that were used on trust are not accepted as an install

#### Scenario: What is recorded when trust is used

- **WHEN** a model's files are used on trust
- **THEN** a record at the default verbosity names the model and says its verification was skipped
