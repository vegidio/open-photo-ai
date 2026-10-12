## Purpose

Measuring what this application's inference actually costs on the machine in front of you, and
reporting it in a form that can be quoted: what each stage of a measurement covers, what the numbers
are not allowed to quietly include, what the report must state about the conditions that produced it,
and how a model that cannot be run, fails, or is interrupted is accounted for without invalidating
the rest of the sweep.

## ADDED Requirements

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

### Requirement: A timing measures inference rather than storing the result

The system SHALL keep the application's image cache **off** by default, so that a reported number
covers running the model and not encoding and writing its result.

A caller MAY turn the cache on, to measure instead what a run costs an ordinary consumer of the
library. Where it is on, **every timed run SHALL still do the work**: no run of a measurement may be
served from the result of an earlier one. A measurement of five runs must be five inferences, not
one inference and four reads.

The report SHALL state which of the two was measured, because the difference is large enough to
change a conclusion and is invisible in the numbers themselves.

#### Scenario: The default measurement

- **WHEN** a model is benchmarked without asking for the cache
- **THEN** the results of the runs are not written to the application's image cache
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
whether the image cache was on, and how many models were selected.

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
