## Purpose

How the window asks for an open photograph to be analysed for Autopilot, which photograph and framing that analysis
sees, what comes back across the boundary, and how an analysis the window no longer wants is stopped.

## ADDED Requirements

### Requirement: The interface can ask for an open photograph to be analysed

The application SHALL let its interface ask for an open photograph to be analysed, naming the enhancement families
the analysis may suggest. The answer SHALL be the enhancements the photograph calls for, among those families only.

Each suggestion SHALL name its family. A suggested upscale SHALL carry the scale the analysis chose for it. No
suggestion SHALL carry a model, a precision, or any other value: which model runs is the user's standing
preference, which the interface holds.

The suggestions SHALL come back in the order the application applies enhancements in, whatever order the interface
named the families in.

An empty set of families SHALL be answered with no suggestions and SHALL NOT be a failure. A photograph that calls
for nothing SHALL likewise be answered with no suggestions rather than a failure.

#### Scenario: A photograph that calls for work

- **WHEN** the interface asks for an open, small, grayscale photograph to be analysed, naming upscale and colorization
- **THEN** a colorization suggestion and an upscale suggestion come back, colorization first
- **AND** the upscale suggestion carries its scale
- **AND** neither names a model or a precision

#### Scenario: A family the interface did not name

- **WHEN** the interface asks for a photograph to be analysed without naming face recovery, and the photograph has
  faces in it
- **THEN** no face-recovery suggestion comes back

#### Scenario: No families named

- **WHEN** the interface asks for a photograph to be analysed and names no families
- **THEN** no suggestions come back, and the request is not a failure

#### Scenario: A photograph that needs nothing

- **WHEN** the interface asks for a photograph to be analysed and it calls for none of the families named
- **THEN** no suggestions come back, and the request is not a failure

### Requirement: An analysis sees the photograph as the user has framed it

The analysis SHALL be made on the photograph as it is framed at the time of the request: the crop, rotation and flip
the interface sends are applied before anything is read from it. A request that sends no framing SHALL be analysed on
the whole photograph.

Faces found by an analysis SHALL be the faces a request to detect faces in that photograph at that framing would
find. A request to detect faces made after an analysis, for the same photograph at the same framing, SHALL be
answered without running the detector a second time.

#### Scenario: A cropped photograph

- **WHEN** the interface asks for a photograph to be analysed with a crop that leaves a small region of it
- **THEN** the analysis is made on that region, and an upscale suggested for it carries the scale that region's size
  calls for

#### Scenario: Faces asked for after an analysis

- **WHEN** a photograph is analysed with face recovery among the families, and the interface then asks for the faces
  in that photograph at the same framing
- **THEN** the faces come back without the detector being run again

### Requirement: Only a photograph the user has opened can be analysed

A request to analyse SHALL name an image the user has opened. A request naming anything else SHALL be refused before
any file is read or any model is fetched. A request made before the application has finished starting SHALL be
refused, and so SHALL a request for an opened photograph whose file can no longer be read.

Each of these refusals SHALL be distinguishable from the others, and from an analysis that failed while it ran.

#### Scenario: An image that was never opened

- **WHEN** the interface asks for an analysis naming an identity the application never admitted
- **THEN** the request is refused as an unknown source, and nothing is read or fetched

#### Scenario: A file removed after it was opened

- **WHEN** the interface asks for an analysis of an opened photograph whose file is no longer on disk
- **THEN** the request is refused as an unreadable source

#### Scenario: Before start-up has finished

- **WHEN** the interface asks for an analysis before the application has finished starting
- **THEN** the request is refused as not ready

### Requirement: An analysis the interface no longer wants can be stopped

The interface SHALL name each analysis it asks for, and SHALL be able to stop an analysis by that name. A stopped
analysis SHALL answer with a refusal that says it was stopped, distinct from a failure, and SHALL NOT answer any
suggestions, including ones already determined.

A stop that reaches the application before the analysis it names has started SHALL still stop that analysis.

A stop naming an analysis that has already finished, or that the application has never heard of, SHALL do nothing
and SHALL NOT be a failure. Stopping one analysis SHALL NOT affect any other.

#### Scenario: Stopped while running

- **WHEN** the interface stops an analysis while its face detection is running
- **THEN** that analysis answers that it was stopped, with no suggestions

#### Scenario: The stop arrives first

- **WHEN** the interface asks for an analysis and stops it, and the stop reaches the application first
- **THEN** the analysis answers that it was stopped, and no model is run for it

#### Scenario: A stop for a finished analysis

- **WHEN** the interface stops an analysis that has already answered
- **THEN** nothing happens, and the stop is not a failure

#### Scenario: Two analyses in flight

- **WHEN** two photographs are being analysed and the interface stops one of them
- **THEN** the other goes on and answers its suggestions

### Requirement: Analysing does not disturb enhancing

An analysis SHALL NOT stop, replace or release an enhancement run or an enhanced result, and an enhancement run SHALL
NOT stop an analysis. More than one analysis SHALL be allowed to be in flight at once.

#### Scenario: An analysis during a run

- **WHEN** a photograph is being enhanced and the interface asks for another photograph to be analysed
- **THEN** the enhancement run goes on, and its result is still served when it finishes

### Requirement: An incomplete analysis answers what it could read

Where one of an analysis's signals could not be read, such as a face detector that could not be fetched, the request
SHALL still answer the suggestions from the signals that were read, and SHALL NOT be a failure. The reason SHALL be
recorded in the application's log. It SHALL NOT be part of the answer, which carries only what the photograph calls
for.

#### Scenario: The face detector cannot be fetched

- **WHEN** a small photograph is analysed with upscale and face recovery among the families, and the face detector
  cannot be fetched
- **THEN** the upscale suggestion comes back and the request is not a failure
- **AND** the log records that the face signal could not be read, and why
