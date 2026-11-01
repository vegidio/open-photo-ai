## ADDED Requirements

### Requirement: A model's hash comes from the live published listing

The application SHALL obtain a model's published files, sizes and hashes from the project's live published listing,
and from nothing else. It SHALL NOT keep a copy of the listing between runs, and SHALL NOT carry a listing inside the
application.

A model that is not installed SHALL be installed only when the listing can be read and names it. Where the listing
cannot be read, a model that is installed and intact SHALL be used as it is (see "An install that is already current
is not repeated"), and a model that is not SHALL fail with an error that names the artifact and says the published
listing could not be read, with nothing transferred.

#### Scenario: The live listing is reachable

- **WHEN** a model install runs and the published listing can be read
- **THEN** the files, sizes and hashes in that listing are the ones the model is installed and verified against

#### Scenario: A model that is not installed, with no network

- **WHEN** a model install runs for an artifact that is not installed, and the published listing cannot be read
- **THEN** the install fails with an error naming the artifact and saying the listing could not be read
- **AND** no transfer is attempted

#### Scenario: Nothing of the listing survives the run

- **WHEN** a run has read the published listing and ended
- **THEN** no copy of the listing is left on disk for a later run to use

### Requirement: An install from an earlier release is adopted without downloading it again

Where a model's files are already on disk at their published sizes but no install record this release recognises
names them, the application SHALL hash each of them against the published listing. It SHALL keep every file that
matches, SHALL download only those that do not, and SHALL then record the install as current. A file that does not
match SHALL NOT be kept.

A model in the folder of an earlier release is gigabytes that are already verified. Discarding them because the
format of the record beside them changed would cost every user a full re-download for nothing.

#### Scenario: A model installed by an earlier release

- **WHEN** a model is acquired whose files an earlier release installed and whose published hashes have not changed
- **THEN** each file is hashed and kept
- **AND** nothing is transferred
- **AND** the install is recorded, so the next acquisition hashes nothing

#### Scenario: A file from an earlier release that no longer matches

- **WHEN** a model is acquired whose file from an earlier release does not match the published hash
- **THEN** that file is downloaded again and verified
- **AND** what an execution provider compiled from the previous weights is removed

#### Scenario: A model from an earlier release, with no network

- **WHEN** a model installed by an earlier release is acquired and the published listing cannot be read
- **THEN** the acquisition fails as for a model that is not installed, since nothing can verify those files

## MODIFIED Requirements

### Requirement: A model is never installed without a published hash

The application SHALL verify every downloaded model file against the hash the published listing reports for it, and
SHALL refuse to install a file for which the listing reports no hash. There SHALL be no path - including an
unreachable listing, an empty listing, an entry that carries no hash, or a declaration of trust in the files already
on disk - by which a model file the application itself obtained is placed on disk without having been checked.

This is a deliberate divergence from the reference implementation, which downloads an unknown model
unverified and logs a warning. It is also a divergence from the reference's debug override, which
suppresses the check on a file it has just downloaded as well as on one already present; here that
override reaches only the second, and the requirement below is its whole contract.

The one path by which an unchecked model file may be **loaded** is a file the operator placed on disk
themselves, in a process that declared at initialization that it trusts such files. That is a
property of what was already there rather than of anything the application fetched, and nothing about
it relaxes the paragraph above. A model installed and verified by an earlier run, used as it is because the listing
cannot be read, is not unchecked: it was checked when it was installed.

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

- **WHEN** the published listing is read and names no file for the artifact
- **THEN** the install fails with an error naming the artifact and saying no published hash is known
- **AND** no transfer is attempted

#### Scenario: A listing entry that carries no hash

- **WHEN** the published listing names a file but publishes no hash for it
- **THEN** that entry is treated as though the file were absent from the listing
- **AND** nothing is installed unverified

### Requirement: An install that is already current is not repeated

Where a model's recorded install names exactly the files the published listing names for it, every recorded hash
equals the listing's, and every recorded file is still present at its recorded size, the application SHALL treat the
model as installed: nothing is transferred, nothing is hashed and nothing is reported.

A model is republished under the same name when it changes. Where a recorded hash differs from the listing's, or the
listing names a different set of files, the model SHALL be replaced as stated in "Installing or replacing one model
leaves other models untouched".

Where the listing cannot be read, a model whose recorded files are all present at their recorded sizes SHALL be used
as it is, with nothing transferred and nothing reported, so that a machine with no network can run every model it has
already installed.

#### Scenario: A model that is already installed and intact

- **WHEN** an install runs for a model whose recorded files are all present at their recorded sizes, and whose
  recorded hashes are the published ones
- **THEN** no transfer is attempted and no progress is reported
- **AND** the install reports success

#### Scenario: A model whose files were removed after installation

- **WHEN** an install runs for a model whose recorded files are missing or the wrong size
- **THEN** the model is installed again

#### Scenario: A model republished under the same name

- **WHEN** an install runs for an installed model whose published hash has changed since it was installed
- **THEN** the model is replaced with the published files and verified
- **AND** what an execution provider compiled from the previous weights is removed

#### Scenario: An installed model with no network

- **WHEN** an install runs for a model whose recorded files are all present at their recorded sizes, and the
  published listing cannot be read
- **THEN** the installed files are used as they are
- **AND** no transfer is attempted and the install reports success

### Requirement: Obtaining the published listing does not delay startup

The application SHALL NOT read the published model listing as part of initialization. The listing
SHALL be read no earlier than the first model install that needs it, and once read SHALL serve every
later install in the same run without being read again.

Where that read fails, later installs in the same run SHALL NOT wait on the listing again for a model that is already
installed and intact. A model that is not installed MAY try the listing again, since nothing else can install it.

Initialization's behaviour, including which failures it reports, SHALL be unchanged by the presence
of this capability.

#### Scenario: An application that initializes and never installs a model

- **WHEN** initialization completes and no model install is ever requested
- **THEN** the published listing is never read

#### Scenario: Several installs in one run

- **WHEN** more than one model is installed in a single run
- **THEN** the published listing is read at most once
- **AND** every install after the first uses what was already read

#### Scenario: Several installed models in a run with no network

- **WHEN** the listing could not be read for one installed model, and a second installed model is acquired later in
  the same run
- **THEN** the second model is used as it is without another attempt to read the listing

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
verified against the hash published for them in every process regardless of it: those are downloaded
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
- **THEN** the archive is verified against the hash published for it, as it is in any other process

## REMOVED Requirements

### Requirement: The hash is resolved from the first available of four sources

**Reason**: The cached copy of the listing and the listing built into the application are removed. The built-in
listing went stale with every publish and had to be regenerated by hand. The case it served, a first install with the
listing unreachable but the files downloadable, cannot happen, since both are served by the same host. An installed
model no longer needs the listing at all when it cannot be read.

**Migration**: "A model's hash comes from the live published listing" replaces it. Installed models keep working
offline through "An install that is already current is not repeated". The orphaned cached copy is deleted on
initialize.
