## MODIFIED Requirements

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

## ADDED Requirements

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
