## MODIFIED Requirements

### Requirement: A photograph is analysed when it is looked at with Autopilot on and nothing done to it yet

The window SHALL ask for a photograph to be analysed when all three of these hold at once:

- the photograph is the current one
- Autopilot is on
- the photograph has never had a list of enhancements

An analysis SHALL be asked for whenever that becomes true, whichever of the three changed last. That includes the
user turning Autopilot on while looking at such a photograph.

A photograph that is open but not current SHALL NOT be analysed, with one exception: an export reaching it. When a
batch export reaches a photograph that has never had a list of enhancements, with Autopilot on, that photograph SHALL
be analysed before it is exported, whether or not it is current. What the analysis suggests SHALL be added to its
list by the same rules as any other analysis, and the export SHALL run what the list then holds. Opening a folder of
photographs SHALL NOT analyse every one of them: each is analysed when the user comes to it, or when an export
reaches it.

A photograph SHALL have at most one analysis in flight. Returning to a photograph whose analysis is still under way
SHALL NOT ask for a second one. An export that reaches a photograph whose analysis is already under way SHALL wait
for that analysis rather than ask for a second one.

#### Scenario: A photograph opened with Autopilot on

- **WHEN** a photograph is opened into an empty window and becomes the current one, with Autopilot on
- **THEN** it is analysed

#### Scenario: Several photographs opened at once

- **WHEN** five photographs are opened at once with Autopilot on
- **THEN** only the one that becomes current is analysed, and each of the others is analysed when the user moves to it

#### Scenario: Autopilot off

- **WHEN** a photograph becomes current with Autopilot off
- **THEN** it is not analysed

#### Scenario: Autopilot turned on over a photograph

- **WHEN** the user turns Autopilot on while looking at a photograph that has never had a list of enhancements
- **THEN** it is analysed

#### Scenario: Back to a photograph still being analysed

- **WHEN** the user moves off a photograph while it is being analysed and comes back before the analysis answers
- **THEN** no second analysis is asked for, and the first one's answer is applied when it arrives

#### Scenario: A never-visited photograph reached by an export

- **WHEN** Autopilot is on, and a batch export reaches a picked photograph the user never looked at
- **THEN** it is analysed, its suggestions are added to its list, and it is exported with them

#### Scenario: An export with Autopilot off

- **WHEN** Autopilot is off and a batch export reaches a photograph that has never had a list of enhancements
- **THEN** it is not analysed, and it is exported as it is framed

#### Scenario: An export reaching an analysis already under way

- **WHEN** a batch export reaches the current photograph while its analysis is still under way
- **THEN** no second analysis is asked for, and the export runs what the first one adds

### Requirement: A failed analysis is reported, and a stopped one is not

An analysis that fails SHALL be reported to the user in a way that does not interrupt what they are doing. This
covers a photograph that cannot be read and an analysis that went wrong while it ran. The photograph's list SHALL be
left as it was.

An analysis asked for by an export that fails SHALL be reported on that export's row instead, which reads Failed and
carries the reason. It SHALL NOT also be reported the usual way. The photograph SHALL NOT be exported, and the batch
SHALL go on to the next row.

An analysis that answers only part of what it was asked, because one of its signals could not be read, SHALL NOT be
reported. What it did answer SHALL be applied.

An analysis that was stopped SHALL NOT be reported, and nothing it had found SHALL be applied.

#### Scenario: An analysis fails

- **WHEN** an analysis cannot be completed
- **THEN** the user is told that Autopilot failed, and the photograph's list is unchanged

#### Scenario: An analysis fails during an export

- **WHEN** an analysis asked for by an export cannot be completed
- **THEN** that export's row reads Failed with the reason, no other notice is shown, no file is written for that
  photograph, and the batch continues

#### Scenario: A signal could not be read

- **WHEN** an analysis answers an upscale but could not fetch the face detector
- **THEN** the upscale is added and nothing is reported
