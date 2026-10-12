## Purpose

What the graphical application's settings surface can know and do: which execution providers the machine in front of
it turned out to offer, where its own log file is and how a user is shown it, and which models each enhancement can
be set to default to. These are the facts the settings dialog is drawn from, and they are answered by the application
whether or not a dialog is open.

## ADDED Requirements

### Requirement: A completed setup reports what this machine can be asked to run on

Once the application has finished setting itself up, it SHALL report which execution providers this machine offers.
The report SHALL name every provider the application can be asked for, including the one that is always available, so
that a list of choices can be built from it without anything outside knowing which providers exist.

The report SHALL describe what was actually installed on this machine, not merely what hardware was detected. A
machine whose graphics adapter is supported but for which no library was published SHALL NOT be reported as offering
that adapter's provider - otherwise the application would offer a choice that can only fall back.

A request that arrives after the application is already set up SHALL be answered with the same report without
installing, verifying or transferring anything again. Setting up is what decides the answer, and asking again is not
a reason to do it twice.

#### Scenario: A machine with an NVIDIA RTX adapter and its libraries installed

- **WHEN** setup finishes on a machine where the CUDA and TensorRT libraries were installed
- **THEN** the report names CUDA and TensorRT among the providers this machine offers
- **AND** it names the CPU provider as well

#### Scenario: Hardware is present but nothing was published for the platform

- **WHEN** setup finishes on a machine with an NVIDIA adapter on a platform no NVIDIA library is published for
- **THEN** the report does not name that adapter's providers
- **AND** the application offers only the providers whose libraries are on this machine

#### Scenario: The report is asked for again

- **WHEN** the application is asked to set up a second time in the same session, after the first succeeded
- **THEN** it answers with the same report
- **AND** nothing is downloaded, expanded, verified or loaded again

### Requirement: The application can say where its log file is without creating anything

The application SHALL be able to report the location of its own log file. Asking SHALL create no directory, open no
file, install no log sink and write no record, so that the question can be asked at any time - including by a screen
that is merely being drawn - without changing anything on disk.

The location SHALL be the one this application actually logs to, so a user directed there finds the file the
application has been writing.

#### Scenario: Asking where the log is

- **WHEN** the application is asked where its log file is
- **THEN** it answers with the path of the file it logs to

#### Scenario: Asking is free of side effects

- **WHEN** the application is asked where its log file is on a machine where it has never run before
- **THEN** it answers with a path
- **AND** no directory and no file has been created at that path

#### Scenario: The location cannot be resolved

- **WHEN** the platform offers no configuration directory to resolve the location from
- **THEN** the application reports that the location could not be determined
- **AND** nothing is created

### Requirement: The log file can be shown to the user in the platform's file manager

The application SHALL be able to open the running platform's own file manager with its own log file selected, so a
user can reach that file without being asked to type or copy its path.

Which file is shown SHALL be decided where the location is resolved rather than supplied by the surface asking for
it, so that the file revealed and the location reported cannot be made to disagree. Revealing an arbitrary path is
not something the application offers.

This SHALL work on each of the application's three shipping platforms, using that platform's own file manager. A
failure to open it SHALL be reported to the caller rather than passing silently, because the caller is the path a
user is being given for diagnosing something else.

#### Scenario: Revealing the log file

- **WHEN** the application is asked to show its log file
- **THEN** the platform's file manager opens with that file selected
- **AND** it is the same file the application reports its log's location to be

#### Scenario: The file manager could not be opened

- **WHEN** the application is asked to show its log file and the platform's file manager cannot be opened
- **THEN** the failure is reported to the caller

### Requirement: The models each enhancement offers are published to the application's own interface

The application SHALL publish, to its own interface, the catalogue of what the library can be asked to run: every
enhancement family, the models within it, each model's display name and the precisions it is published in, and the
parameters that model takes with the bounds a control may offer.

It SHALL be the library's catalogue unchanged. The application's interface SHALL NOT carry a list of model names,
labels or bounds of its own - that duplication is what lets a control offer a model that no longer exists or a value
the library would refuse.

Every family SHALL be published, including families with no adjustable parameter, so that "this enhancement has
nothing to configure" is something the interface is told rather than something it infers from an absence.

#### Scenario: Reading what a family offers

- **WHEN** the interface asks for the catalogue
- **THEN** it receives every enhancement family with the models each one offers, their display names and their
  published precisions

#### Scenario: A family with nothing to adjust

- **WHEN** the interface asks for the catalogue
- **THEN** a family whose models take no adjustable parameter is present with an empty parameter list, rather than
  being left out
