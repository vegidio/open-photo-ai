# performance-benchmark Specification

## Purpose

Measuring what this application's inference actually costs on the machine in front of you, and
reporting it in a form that can be quoted: what each stage of a measurement covers, what the numbers
are not allowed to quietly include, what the report must state about the conditions that produced it,
and how a model that cannot be run, fails, or is interrupted is accounted for without invalidating
the rest of the sweep.

## Requirements

### Requirement: A model is measured in four stages, and only two of them are timed

The system SHALL measure one model at a time, against one image, in four stages: an untimed warm-up,
a timed cold start, a number of timed runs, and the statistics over those runs.

The warm-up SHALL be what puts the model on disk where it is not there already, and SHALL NOT be
timed. The cold start SHALL be measured **after** the warm-up, so that it covers building a session
and running one inference and never covers a download. A cold start that included a
multi-hundred-megabyte transfer would not describe session construction, and a number taken on a
fresh machine could not be compared with one taken on a warm one.

Where the caller asks for no warm-up at all, the cold start MAY include a transfer, and the report
SHALL say so rather than presenting the number as though it were comparable.

The number of timed runs and of warm-up runs SHALL be the caller's to choose, with at least one
timed run required.

#### Scenario: A model measured with the default stages

- **WHEN** a model is benchmarked with a warm-up and a number of timed runs
- **THEN** the reported cold start covers building a session and one inference
- **AND** it does not include the time taken to obtain the model

#### Scenario: A model not on disk when the sweep starts

- **WHEN** a model whose files are absent is benchmarked with at least one warm-up run
- **THEN** the transfer happens during the warm-up
- **AND** none of the reported numbers includes it

#### Scenario: A measurement asked for with no warm-up

- **WHEN** a model is benchmarked with the warm-up count set to zero
- **THEN** the report states that the cold start may include obtaining the model

### Requirement: Every model's cold start starts from nothing

Before the cold start of each model, the system SHALL release every resident session, so that the
session the cold start builds is genuinely built rather than found.

This SHALL also be done between models, so that the memory held by one model's sessions is not still
held while the next is measured, and so that a run's inability to use the requested execution
provider is attributed to the model that actually hit it.

Where a release is asked for and nothing is in use, the next measurement SHALL pay the full
construction cost. A cold start that reused a session the warm-up had already built would report
the cost of a lookup and present it as the cost of a build — the single most misleading number this
capability could produce.

#### Scenario: The cold start after a warm-up

- **WHEN** a model's warm-up has built a session and the cold start is then measured
- **THEN** the cold start includes building a session again

#### Scenario: Moving from one model to the next

- **WHEN** one model's measurement finishes and the next model's begins
- **THEN** no session built for the first is still resident when the second is measured

### Requirement: A model whose input is another model's result is given it once, and is not timed producing it

Where a selected model cannot do any work without a result only another model can produce — a face-recovery model,
which restores the faces it is given and finds none of its own — the system SHALL obtain that result **once**, before
the sweep begins, and SHALL supply it to every model that needs it.

Without it the measurement is of nothing. A face-recovery run over an empty selection composites no face and returns
almost immediately, so a sweep would report a sub-millisecond time for a model that takes seconds on a real
photograph, and would report it as a success — which is worse than the refusal it replaced, because a failure is
visible and a plausible wrong number is not.

The auxiliary run SHALL be made at the sweep's own precision rather than at a pinned one, because that is what the
application does: a run that restores at one precision detects at the same one, and pinning would measure a
combination the application never produces.

Its cost SHALL NOT be charged to any model's timings, and it SHALL NOT be repeated per model. Each model's cold start
already starts from nothing, so a result obtained per model would pay for a second model's full session build inside
another model's measurement — untimed but real wall clock, and on TensorRT larger than everything else in the sweep.

Where the auxiliary run fails, or succeeds and finds nothing to work with, every model that depended on it SHALL fail
with that reason. Failing is the required outcome rather than measuring the empty case: a sweep of a photograph with
no face in it has nothing to say about a face-recovery model, and saying it quickly is the defect this requirement
exists to prevent.

Models that do not need the result SHALL be unaffected — neither delayed by it nor failed by its failure — and a sweep
that selected none of the models that need it SHALL NOT make the auxiliary run at all.

The report SHALL state what the auxiliary run found, alongside the other conditions that produced the numbers, since
the number of faces is what a face-recovery model's timings scale with.

#### Scenario: A sweep including a face-recovery model

- **WHEN** a sweep selects a face-recovery model and an image with faces in it
- **THEN** the faces are obtained once before the sweep and supplied to that model
- **AND** the time taken to obtain them appears in no model's measurement

#### Scenario: A sweep including two face-recovery models

- **WHEN** a sweep selects two models that both need the same auxiliary result
- **THEN** it is obtained once and supplied to both

#### Scenario: An image with no faces in it

- **WHEN** a sweep selects a face-recovery model and an image in which the auxiliary run finds no face
- **THEN** that model is reported as failed with that reason, rather than measured

#### Scenario: A sweep needing no auxiliary result

- **WHEN** a sweep selects only models that need no other model's result
- **THEN** no auxiliary run is made

#### Scenario: The report of a sweep that made one

- **WHEN** a sweep that made an auxiliary run produces a report
- **THEN** the report states what it found, beside the image, the provider, the precision and the other conditions

### Requirement: A timing measures inference rather than storing the result

The system SHALL keep the application's run cache **off** by default, so that a reported number
covers running the model and not encoding and writing its result.

A caller MAY turn the cache on, to measure instead what a run costs an ordinary consumer of the
library. Where it is on, **every timed run SHALL still do the work**: no run of a measurement may be
served from the result of an earlier one. A measurement of five runs must be five inferences, not
one inference and four reads.

The report SHALL state which of the two was measured, because the difference is large enough to
change a conclusion and is invisible in the numbers themselves.

#### Scenario: The default measurement

- **WHEN** a model is benchmarked without asking for the cache
- **THEN** the results of the runs are not written to the application's run cache
- **AND** the report states that the timings are inference rather than encoding

#### Scenario: A measurement with the cache on

- **WHEN** a model is benchmarked with the cache turned on
- **THEN** each timed run performs its own inference rather than being served from a previous run
- **AND** the report states that the timings include what the cache costs

### Requirement: The timed runs are reported as a distribution, not as one number

For each model that completed, the system SHALL report the cold start, and over the timed runs the
minimum, the median, the mean, the maximum and the sample standard deviation.

The standard deviation SHALL be the sample deviation, since the runs are a sample of the runs the
machine could have produced; with a single timed run it SHALL be zero rather than undefined.

The system SHALL also report a throughput figure computed from the **input** megapixels over the
median run, and the report SHALL say that it is the input that it is computed from. Input, because
it is the one thing every model has in common: an upscaler's output is many times its input and a
denoiser's is the same size, so a figure over the output would not compare two models at all.

Where a model produces an image, its dimensions SHALL be reported, so that a throughput figure can
be read beside the work it describes.

#### Scenario: A completed model

- **WHEN** a model completes its timed runs
- **THEN** the cold start, minimum, median, mean, maximum and sample standard deviation are reported
- **AND** a throughput figure over the input megapixels and the median run is reported beside them

#### Scenario: A single timed run

- **WHEN** a model is benchmarked with exactly one timed run
- **THEN** the standard deviation is reported as zero
- **AND** the minimum, median, mean and maximum are all that one run's duration

### Requirement: The report states the conditions that produced the numbers

The report SHALL describe, before the numbers, everything that would change them: the image measured
and its size, the execution provider requested and which providers this machine supports, the
precision, how many runs were timed and how many were warm-ups, the parameters the models were given,
whether the run cache was on, and how many models were selected.

A table of durations with no such header is not quotable: pasted into an issue it cannot be told
apart from one taken at a different precision, on a different provider, or with the cache on, and
every one of those differences is larger than the differences the table exists to show.

The report SHALL also state that the numbers are comparable within one invocation and not across
machines, processes, or a thermally throttled one, since nothing in this capability controls for any
of that.

#### Scenario: Any report

- **WHEN** a sweep produces a report
- **THEN** it states the image, the requested provider and the machine's support, the precision, the
  run counts, the parameters, the cache state and the number of models selected

#### Scenario: A report of a sweep on a machine with no supported GPU provider

- **WHEN** a sweep runs on a machine that supports no GPU execution provider
- **THEN** the report says which providers the machine supports, rather than only which was requested

### Requirement: A model that did not run on the provider that was asked for is reported

Where a measured run reports having been executed on an execution provider other than the one
requested, the system SHALL report that model as having been downgraded, naming both providers, and
SHALL say that those rows are timings for what actually ran rather than for what was asked for.

Without this the most misleading outcome available is silent: a model that could not be opened on a
GPU is completed on the CPU, returns the same image in the same way, and is several times slower —
which is indistinguishable, in a table, from a GPU that is slow.

A model whose run reports that nothing was executed SHALL NOT be reported as having run on the
requested provider.

#### Scenario: A model downgraded to the CPU

- **WHEN** a model is benchmarked asking for a GPU provider and its run reports that it executed on
  the CPU
- **THEN** the report names that model, the provider requested and the provider it ran on
- **AND** it says the row is a timing of what ran rather than of what was asked for

#### Scenario: A model served on the provider requested

- **WHEN** a model's run reports that it executed on the provider that was requested
- **THEN** no downgrade is reported for it

### Requirement: A model the application declines to run is skipped rather than counted as a failure

Where the application declines to run an operation because it has no pipeline for it, the system
SHALL NOT report that as a failed measurement.

A sweep that was not told which models to measure SHALL report only the models that were measured,
and SHALL state how many were skipped and that naming one shows why. A model the caller names
explicitly SHALL be attempted whatever the application makes of it, so that the refusal and the
reason the application gives for it are reported rather than the request being silently dropped.

Which operations the application will run is the application's own answer and changes as it gains
pipelines. The system SHALL NOT keep a list of its own, and SHALL NOT describe a model as runnable or
not outside the outcome of attempting one — so that a model that becomes runnable is measured by the
same sweep with nothing here changed, and nothing here is left to remove once all of them are.

Being declined SHALL cost no transfer: an operation the application will not run must not first
obtain its weights.

#### Scenario: A sweep with no models named

- **WHEN** a sweep is asked for without naming any model
- **THEN** the models the application ran are measured and reported
- **AND** the models it declined are neither reported as failures nor counted as measured
- **AND** the report states how many were skipped and that naming one shows why

#### Scenario: A model named explicitly that the application declines

- **WHEN** a model the application has no pipeline for is named explicitly
- **THEN** the reason the application gives is reported
- **AND** nothing is transferred on its behalf
- **AND** the sweep's exit status does not report it as a failure

#### Scenario: A model the application begins to run

- **WHEN** a model the application declined in an earlier release is swept after it gains a pipeline
- **THEN** it is measured and reported like any other model, with no change to the system

### Requirement: Listing and selecting models needs no runtime and no network

The system SHALL list the available models, and SHALL reject a model name it does not recognise,
without initializing the application, obtaining anything, or contending with another running process.

A name that does not exist must fail in milliseconds. Discovering a typo after a multi-gigabyte
transfer, or being refused because another process of this application is running when all that was
asked for was a list, are both failures of this requirement.

A model named more than once SHALL be measured once.

#### Scenario: Listing while another instance of the application is running

- **WHEN** the available models are listed while the application's user interface is running
- **THEN** the list is produced

#### Scenario: A name that matches no model

- **WHEN** a sweep names a model that does not exist
- **THEN** it fails immediately, naming the unrecognised name and how to see the available models
- **AND** nothing is obtained and no measurement is taken

#### Scenario: A model named twice

- **WHEN** a sweep names the same model twice
- **THEN** it is measured once

### Requirement: One model's failure does not end the sweep

Where a model cannot be measured — its weights cannot be obtained, no session can be built for it,
or a run fails — the system SHALL record the failure against that model, continue with the models
that remain, and report every failure with its reason after the results.

A sweep is a long job. Ending it on the first failure would throw away every measurement taken before
it, and on a full sweep that is hours of work discarded for a fault in one model.

#### Scenario: A model that cannot be measured

- **WHEN** one model of several fails during its measurement
- **THEN** it is reported as failed with the reason
- **AND** the remaining models are still measured and reported

### Requirement: An interrupted sweep reports what it finished

Where the operator interrupts the sweep, the system SHALL stop the run in flight, report the results
for the models that completed, and mark the model that was interrupted as interrupted rather than as
failed.

An interruption is not a fault in a model. Reporting it as one would send somebody to debug a model
that is working, and would make a cancelled run indistinguishable from a broken one.

#### Scenario: Interrupting a sweep in progress

- **WHEN** the operator interrupts a sweep during a model's measurement
- **THEN** the run in flight stops
- **AND** the results for the models that had completed are reported
- **AND** the interrupted model is marked as interrupted rather than failed

### Requirement: The exit status says whether anything was wrong with a model

The system SHALL exit reporting failure where a model failed to be measured, or where the sweep was
interrupted, and reporting success otherwise.

A model that could not be run because the application has no pipeline for it SHALL NOT count as a
failure: nothing about the sweep went wrong, and a status that said otherwise could never be used as
a check.

#### Scenario: A sweep in which every measured model completed

- **WHEN** a sweep completes with no failure and no interruption
- **THEN** it exits reporting success

#### Scenario: A sweep in which a model failed

- **WHEN** a sweep completes and one model failed to be measured
- **THEN** it exits reporting failure, saying how many models failed

#### Scenario: A sweep of models the application declined

- **WHEN** a sweep names only models the application has no pipeline for
- **THEN** the reason is reported for each
- **AND** it exits reporting success

### Requirement: The results are available in a form a program can read

The system SHALL be able to produce its report as a machine-readable document instead of the
rendered one, carrying the same facts: the conditions the sweep ran under, and for each model its
outcome — the durations and statistics where it completed, and the reason where it was declined,
failed or interrupted.

Comparing two sweeps is the reason this capability exists, and two rendered tables can only be
compared by a person reading them both. The machine-readable form SHALL be the whole of what is
written to the output, so that it can be consumed directly, with progress and diagnostics kept
separate from it.

#### Scenario: A sweep asked for in machine-readable form

- **WHEN** a sweep is asked to report as a machine-readable document
- **THEN** the document carries the conditions of the run and every model's outcome
- **AND** the rendered table is not written to the same output

#### Scenario: A machine-readable report of a sweep with a declined model and a failure

- **WHEN** a sweep that named a model the application declined and one that failed reports in
  machine-readable form
- **THEN** each model's outcome is distinguishable, with the reason carried for both

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
