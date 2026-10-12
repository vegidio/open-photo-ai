## ADDED Requirements

### Requirement: A sweep in progress says what it is doing

While a sweep is running, the system SHALL report which model is being measured, its position in the
sweep, and which stage of that model's measurement is in progress — obtaining the model, warming up,
the cold start, or which of the timed runs.

It SHALL also report how far the work in flight has got, so that a stage which takes minutes is
visibly making progress rather than merely unfinished.

A benchmark is a long job with nothing to show for itself until it is over: a single model on a CPU
is minutes of silence, and a full sweep is hours of it. Silence and a hang are indistinguishable, and
an operator who cannot tell them apart interrupts a run that was working.

#### Scenario: A model being measured

- **WHEN** a sweep is measuring a model
- **THEN** that model is named, with its position among the models selected
- **AND** the stage in progress is stated, and the timed runs are counted as they are taken

#### Scenario: A stage in progress

- **WHEN** a stage of a measurement is under way
- **THEN** how far through that stage the work has got is reported

### Requirement: Obtaining a model is visible while it happens

Where a model's files are not on disk and the sweep obtains them, the system SHALL report that this
is what is happening, naming the model, for as long as it takes.

This is the longest unattended wait the capability has — hundreds of megabytes for one model and
several gigabytes for a sweep of them — and it happens inside the warm-up, where nothing about the
measurement itself has begun. An operator who is told a model is being measured, and then waits
several minutes for what is documented as a sub-second run, has been told the wrong thing.

#### Scenario: A model that is not on disk

- **WHEN** a sweep reaches a model whose files must be obtained first
- **THEN** it reports that the model is being obtained, naming it
- **AND** it reports the progress of that transfer while it runs

### Requirement: Watching a sweep does not change what it measures

Reporting progress SHALL NOT alter the numbers reported. Nothing outside the measurement may be
written while a timed section is in progress, and what a measurement is charged for within one
SHALL be bounded by work too small to be distinguished from the run-to-run variation the report
already publishes.

The report — rendered or machine-readable — SHALL carry the same facts whichever way the sweep was
watched, so that a figure taken while watching and one taken from a redirected run are the same
figure.

The value of this capability is entirely in whether its numbers can be trusted. A progress display
that quietly charged the models for drawing itself would be the most expensive kind of wrong: the
numbers would still look plausible, and nothing in the report would say which of them included it.

#### Scenario: A timed run while progress is displayed

- **WHEN** a timed run is in progress and the sweep is being watched
- **THEN** nothing is written to the display from within that run
- **AND** the duration reported for it is what it would have been unwatched

#### Scenario: The same sweep watched and unwatched

- **WHEN** one sweep is run while watched and another is run without a display
- **THEN** both produce the same report content for the same outcomes
- **AND** neither is distinguishable from the other by its numbers beyond ordinary run-to-run variation

### Requirement: The time remaining is estimated only where it can be

Where the system reports an estimate of the time remaining, that estimate SHALL be derived from
measurements already taken in the sweep, and SHALL NOT be offered before there is one to derive it
from.

An estimate SHALL NOT be extrapolated across models. What one model costs says close to nothing
about what the next one costs — they differ by more than an order of magnitude — so a figure spanning
them would be presented with an authority it does not have, which is worse than showing none.

#### Scenario: Before any timed run has completed

- **WHEN** a model's first timed run has not yet finished
- **THEN** no estimate of the time remaining is offered for it

#### Scenario: Partway through a model's timed runs

- **WHEN** some of a model's timed runs have completed and others remain
- **THEN** the estimate offered covers that model's remaining runs
- **AND** it does not claim to cover the models that follow it

### Requirement: A sweep is watchable without a terminal, and readable without a display

Where the output is not going to a terminal, the system SHALL report its progress as plain lines
rather than as a display, and SHALL still report it rather than falling silent.

The operator SHALL be able to ask for the plain form even where a terminal is available.

The machine-readable report SHALL never be interleaved with progress of either form: progress and
the report SHALL be separable by their destination alone, so that a sweep whose report is captured
to a file is still watchable while it runs and the captured file is still parseable.

Redirecting a sweep's report to a file is the ordinary way a sweep worth keeping is run, and losing
either the progress or the file to the other is a failure of this requirement.

#### Scenario: A sweep whose progress has no terminal

- **WHEN** a sweep runs with its progress destination redirected
- **THEN** progress is reported as plain lines
- **AND** no display is drawn

#### Scenario: A sweep whose report is captured while it is watched

- **WHEN** a sweep's report is captured to a file on a terminal
- **THEN** the sweep remains watchable as it runs
- **AND** the captured file contains the report and none of the progress

#### Scenario: The plain form asked for explicitly

- **WHEN** an operator asks for the plain form on a terminal
- **THEN** progress is reported as plain lines rather than as a display

### Requirement: What a sweep has finished stays readable

Each model's outcome SHALL be reported as soon as that model is finished, in a form that remains on
screen as the sweep continues and after it has ended, rather than being replaced by the model that
follows it.

A display that showed only the model in flight would leave an operator who watched a two-hour sweep
with nothing to show for the first hour and fifty minutes of it until the final report was rendered —
and nothing at all if the sweep was interrupted before that point.

#### Scenario: One model finishing during a sweep

- **WHEN** a model's measurement ends during a sweep
- **THEN** its outcome is reported immediately
- **AND** that report is still readable while the models after it are measured

#### Scenario: A sweep that has ended

- **WHEN** a sweep finishes or is interrupted
- **THEN** the outcomes reported for the models it finished are still on screen
- **AND** nothing of the progress display is left occupying the screen
