## ADDED Requirements

### Requirement: An analysis can be narrowed to a set of families

The system SHALL let the caller of an analysis name the enhancement families it wants checked, or name none.

Where no set is named, the system SHALL check every family, exactly as an analysis did before the set existed.

Where a set is named, the system SHALL suggest only families in that set. It SHALL NOT read any signal that serves only
families outside the set. The face signal SHALL NOT run unless face recovery is in the set. A signal that more than one
family needs SHALL be read when any one of them is in the set, and SHALL still suggest only the families in the set.

An empty set SHALL check nothing. It SHALL return no suggestions, report the analysis complete, and not be a failure.
An empty set does not mean "no preference": a caller that asks for no families is not charged for every one.

A family in the set that no signal can suggest, such as face detection, SHALL match nothing and SHALL NOT be an error.

A signal that was not read because its families were not asked for SHALL NOT make the analysis incomplete. Only a
signal the analysis tried to read and could not can make it incomplete.

Narrowing SHALL NOT change anything else: suggestions still come back in the canonical family order, a cancelled
analysis still returns nothing, an image with no area still calls for nothing, and a family the application cannot run
is still never suggested.

The record the system keeps of each analysis SHALL name the families it was asked to check, or record that it was not
narrowed. Without that, a user's log cannot tell a family that was never checked from one that was checked and needed
nothing.

The reference implementation has no way to narrow an analysis and always checks every family. Narrowing is new rather
than ported; an analysis that is not narrowed behaves as the reference does.

#### Scenario: No set is named

- **WHEN** an image is analysed without a set of families
- **THEN** every family is checked, and the suggestions are the same as for an analysis before narrowing existed

#### Scenario: A set of two families

- **WHEN** an image that calls for upscaling, colorization, light adjustment and face recovery is analysed with the
  set upscale and colorization
- **THEN** exactly upscale and colorization come back, in the canonical family order
- **AND** no face detection is run

#### Scenario: Face recovery is not asked for and the detection model cannot be obtained

- **WHEN** an image is analysed with a set that does not include face recovery, and the face signal's model cannot be
  installed or opened
- **THEN** the analysis is reported complete

#### Scenario: Face recovery is asked for and the detection model cannot be obtained

- **WHEN** an image is analysed with a set that includes face recovery, and the face signal's model cannot be
  installed or opened
- **THEN** the suggestions from the other families in the set still come back
- **AND** the result reports that the analysis was incomplete, and why

#### Scenario: A family whose signal also serves another family

- **WHEN** a blurred, grainy photograph is analysed with the set sharpen
- **THEN** sharpen comes back and denoise does not

#### Scenario: An empty set

- **WHEN** an image is analysed with an empty set of families
- **THEN** no suggestion comes back
- **AND** no signal is read and no detection is run
- **AND** the analysis is not reported as a failure or as incomplete

#### Scenario: A set naming only face detection

- **WHEN** an image is analysed with a set holding only face detection
- **THEN** no suggestion comes back, and the analysis is not reported as a failure or as incomplete

#### Scenario: A narrowed analysis is cancelled

- **WHEN** an analysis narrowed to face recovery is cancelled while the face signal is running
- **THEN** the call fails with a cancellation, and no suggestions come back

#### Scenario: The record of a narrowed analysis

- **WHEN** an image is analysed with a set of families while a sink is installed
- **THEN** a record at the default verbosity names the families asked for, beside the families suggested
