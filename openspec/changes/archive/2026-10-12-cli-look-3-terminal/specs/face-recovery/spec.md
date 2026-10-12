## MODIFIED Requirements

### Requirement: A chain's detection and the chain itself are reported as one run

The library SHALL let a caller report a chain's face detection and the chain that follows it as one run, whose figure
for the whole run moves forward once from start to exactly 1, rather than as two runs each filling from the start.
Every front end that runs a face recovery SHALL get the same mapping from the library, not reimplement it.

Every enhancement of the chain SHALL own an equal share of the run's figure: with `N` enhancements, each owns `1/N`,
in the order they are applied. The detection SHALL NOT count as an enhancement of its own. Where the detection did work
(its model was fetched or run), its reports SHALL occupy the first fifth of the first enhancement's share, and that
enhancement's own reports SHALL occupy the rest of its share. Every other enhancement's reports SHALL keep their share
unchanged. Where the face recovery is the first enhancement applied, the face recovery's share therefore covers the
detection and the recovery together. Where the detection's result was already known, the detection SHALL report
nothing and the chain's reports SHALL be passed on unchanged, so that re-running a chain over a photograph whose faces
are already known does not jump forward at its start.

A report from the detection SHALL be attributed to the face recovery it was made for. It SHALL carry that face
recovery's position in the chain as it is applied, and the chain's length, not the position and length of the
detection's own single-operation run. The chain's own reports SHALL keep the position and length they already carry.

#### Scenario: A detection that did work, then the chain

- **WHEN** a chain of a face recovery and an upscale is reported as one run, and the detection runs its model
- **THEN** the detection's reports go from the start to one tenth of the run, the face recovery's reports go from one
  tenth to one half, and the upscale's reports go from one half to exactly 1
- **AND** the figure never decreases across the handover

#### Scenario: A detection whose result was already known

- **WHEN** a chain with a face recovery and an upscale is reported as one run, and the detection's result was already
  known
- **THEN** nothing is reported for the detection, the face recovery's reports go from the start to one half, and the
  upscale's reports go from one half to exactly 1

#### Scenario: Equal shares whatever the chain's length

- **WHEN** a chain of a denoise, a face recovery and an upscale is reported as one run
- **THEN** each enhancement's reports end at its own third of the run, whether or not the detection did work

#### Scenario: A face recovery applied after another enhancement

- **WHEN** a chain of a denoise and a face recovery is reported as one run, and the detection runs its model
- **THEN** the detection's reports occupy the first fifth of the denoise's share, since the detection runs before the
  whole chain, and the figure never decreases

#### Scenario: The detection is counted as the face recovery's step

- **WHEN** a chain of a face recovery and an upscale is reported as one run, and the detection runs its model
- **THEN** every report from the detection says it concerns the first of two operations, the position of the face
  recovery in that chain
- **AND** the chain's reports say the face recovery is the first of two and the upscale the second of two
