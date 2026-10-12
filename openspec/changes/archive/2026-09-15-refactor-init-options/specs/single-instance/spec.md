## MODIFIED Requirements

### Requirement: The refusal names the process holding the claim

A refused initialization SHALL report which application holds the claim — the identity that process
declared when it initialized — and the holding process's identifier, so that the user is told what to
close rather than only that something is running.

Where the holder's identity cannot be read, initialization SHALL still be refused, reporting that the
application is already running without naming the holder. The recorded identity SHALL NOT decide
whether a process may proceed: an absent, empty, truncated or unrecognisable record is a refusal that
names no holder, never a grant.

The identity is whatever the holding process declared rather than one of a fixed set of applications.
This project's own binaries declare `gui`, `cli` and `perf`, and a refusal naming one of those SHALL
read exactly as it did before; an application embedding this library declares its own, and a refusal
names that.

#### Scenario: The holder is named

- **WHEN** initialization is refused because the graphical application holds the claim
- **THEN** the error identifies the graphical application and the holding process's identifier

#### Scenario: An embedding application holds the claim

- **WHEN** initialization is refused because a process that declared its own identity holds the claim
- **THEN** the error names that identity and the holding process's identifier

#### Scenario: The holder's identity cannot be read

- **WHEN** initialization is refused and the recorded holder identity is missing, empty or
  unrecognisable
- **THEN** initialization is still refused
- **AND** the error reports that the application is already running without naming a holder

## ADDED Requirements

### Requirement: An application declares who it is, or is known by the name it initialized under

Initialization SHALL accept an identity for the process, and SHALL NOT require one. Where none is
declared, the application name the caller initialized under SHALL be used as the identity.

A default had to be chosen and this is the only one that invents nothing. Any fixed alternative
either names the caller as a binary it is not — recording "the command-line application" for a
desktop application that forgot to say otherwise — or introduces a placeholder that tells a user
nothing about what to close. The name the caller has already supplied is in hand, is what every
directory of that installation is resolved from, and names nothing the caller is not. It is not
exempt from the rule below: a name may be a legible directory segment and still not be recordable as
an identity, and where it is not, initialization is refused rather than the name being coped with.

#### Scenario: An application that declares an identity

- **WHEN** a process initializes declaring an identity
- **THEN** that identity is what a refusal naming this process reports
- **AND** it is what each of this process's log records carries

#### Scenario: An application that declares none

- **WHEN** a process initializes without declaring an identity
- **THEN** the application name it initialized under is used as its identity

### Requirement: An identity that would not survive being recorded is refused

The recorded holder is one line naming the identity and the process identifier, read back by taking
the identity as the text before the first space. An identity SHALL therefore be refused where it is
empty, contains whitespace, or contains control characters, and initialization SHALL fail naming what
was rejected and why.

This SHALL hold for the identity a process is known by, whether it declared one or is known by the
name it initialized under. A name is validated as a directory segment, which permits whitespace the
record cannot carry, so the defaulted identity is subject to this rule rather than exempt from it.

Refusing at the point of declaration rather than accepting and coping is what keeps the failure
legible. An identity containing a space is not an error anywhere it is written; it is an error only
when someone else reads it back, and by then the only symptom is that a refused user is told
something is running without being told what — the exact outcome the holder record exists to prevent,
reappearing as a silent degradation rather than as a fault anyone can find.

#### Scenario: An identity containing a space

- **WHEN** a process initializes declaring an identity containing a space
- **THEN** initialization fails, naming the rejected identity and the reason
- **AND** no claim is taken and nothing is installed

#### Scenario: An empty identity

- **WHEN** a process initializes declaring an empty identity
- **THEN** initialization fails, naming the reason

#### Scenario: An identity containing a control character

- **WHEN** a process initializes declaring an identity containing a line break or other control
  character
- **THEN** initialization fails, naming the reason

#### Scenario: A name that cannot be recorded as an identity

- **WHEN** a process initializes declaring no identity, under a name that is a usable directory
  segment but could not be recorded as an identity
- **THEN** initialization fails, naming the rejected name and the reason
- **AND** no claim is taken and nothing is installed

#### Scenario: A recorded identity round-trips

- **WHEN** a process declares an accepted identity, takes the claim, and another process is refused
  by it
- **THEN** the refusal names exactly the identity that was declared
