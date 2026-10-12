## ADDED Requirements

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

## MODIFIED Requirements

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
