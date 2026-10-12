# run-cache Specification

## Purpose

Keeping what a run produced so that repeating it is a read rather than a run - the result of each step
of an enhancement chain, and the result of an analysis that is not a picture at all - what is stored
and under what identity, how the two kinds of entry are kept from being read as one another, how long
an entry is worth keeping, what happens when the store cannot be opened or a write is refused, and
when the cache is bypassed altogether.

## Requirements

### Requirement: The result of each step of a chain is kept and reused

The system SHALL keep the result of each operation of a chain, and SHALL serve a later request for the
same result from what was kept rather than running the model again.

An entry SHALL be identified by the identity of the image the operation was applied to together with
the operations applied so far, so that the same operation applied after different operations is a
different entry. That identity is the one an enhanced image already carries, so a result fed back in
for further enhancement looks up exactly the entry its own pixels were stored under.

A result served from the cache SHALL be the same image the run that stored it produced: the same
dimensions, the same bits per channel, and the same pixels. Storing and serving SHALL NOT alter the
picture, since a caller cannot tell a served result from a computed one and would be shown a
degraded image as an enhancement.

The execution provider a result was computed on SHALL NOT distinguish entries. The same enhancement
is the same enhancement whichever processor produced it, and a caller that needs each provider to
actually run - measuring what one costs - turns the cache off for that run.

#### Scenario: The same chain run twice

- **WHEN** an image is run through a chain, and the identical chain is then run over the same image
  again
- **THEN** the second run produces the same result as the first
- **AND** no model is run for the operations whose results were kept

#### Scenario: An operation repeated after a different one

- **WHEN** one operation is run after a first operation, and then the same operation is run after a
  different first operation
- **THEN** the two are stored and served as different results

#### Scenario: A chain extended by one operation

- **WHEN** a chain of two operations is run, and then a chain of the same two followed by a third
- **THEN** the first two operations are served from what the earlier run stored
- **AND** only the third operation runs

#### Scenario: A result at a depth other than the one that was stored

- **WHEN** a chain is run asking for a result at one bit depth, and the same chain is then run over
  the same image asking for another
- **THEN** the second run is not served the first run's result
- **AND** it produces a result at the depth it asked for

#### Scenario: A result computed on one processor and requested on another

- **WHEN** a chain is run on one execution provider and the same chain is then run asking for a
  different one
- **THEN** the stored result is served rather than the model being run again

### Requirement: A stored result is kept for a day and its space is reclaimed

An entry SHALL stop being served 24 hours after it was stored. A day covers a working session, which
is the span over which somebody re-runs the same enhancement on the same photograph; past that the
pixels are cheaper to recompute than to keep.

The space held by entries that are no longer served SHALL be reclaimed without the user asking, and
SHALL NOT require the application to be reinstalled or its configuration directory to be cleared by
hand.

Nothing SHALL bound the store by size. What is stored is whole enhanced photographs, so a single
session can hold several gigabytes until that space is reclaimed; a size budget with eviction is a
later change, and this requirement states the current bound so that the absence of one is recorded
rather than assumed.

#### Scenario: An entry older than a day

- **WHEN** a chain is run whose result was stored more than 24 hours earlier
- **THEN** the stored result is not served
- **AND** the operation runs again

#### Scenario: Space held by expired entries

- **WHEN** the application starts and the store holds entries that are no longer served
- **THEN** the space they hold is reclaimed

### Requirement: Storing a result never fails a run

A result that cannot be stored SHALL NOT fail the run that produced it, and SHALL NOT be withheld
from the caller. The pixels the caller asked for have already been computed; a disk that is full or
read-only is the cache's problem, and the cost of carrying on is a re-run later rather than a wrong
image or finished work thrown away.

Every failure to read SHALL be a miss, so the operation runs and produces a correct result. This
includes a stored entry that can no longer be read back as an image - a truncated write, an
interrupted shutdown - which SHALL be treated as though it were not there.

#### Scenario: A result that cannot be stored

- **WHEN** an operation's result cannot be written to the store
- **THEN** the run succeeds and returns that result
- **AND** the following operations of the chain still run

#### Scenario: A stored entry that can no longer be read as an image

- **WHEN** a chain is run whose stored result is unreadable
- **THEN** the operation runs again and produces a correct result

### Requirement: The cache degrades rather than failing, and says what is behind it

A store that cannot be opened SHALL NOT fail initialization. Losing the cache costs speed and nothing
else, and an application that would not start because its cache directory was locked would be
refusing to do work it is perfectly able to do.

Where the store cannot be opened on disk, the system SHALL keep results for the life of the process
instead. Where neither is possible, it SHALL run with no cache at all, recomputing every operation.

The system SHALL report which of the three is in force, so that an application can tell a user - or
its own telemetry - that a run was slower because its cache was degraded. Without that, "some runs
are silently uncached" is unknowable from outside the machine.

Initializing the application more than once in one process SHALL NOT fail on account of the cache,
and SHALL NOT discard results already stored in that process.

#### Scenario: A cache directory that cannot be opened

- **WHEN** the application initializes and its cache directory cannot be opened
- **THEN** initialization succeeds
- **AND** it reports that results are being kept in memory rather than on disk

#### Scenario: No cache available at all

- **WHEN** neither store can be opened
- **THEN** initialization succeeds and reports that there is no cache
- **AND** every operation of every run is computed

#### Scenario: An application initialized twice

- **WHEN** the application is initialized a second time in one process without being torn down
- **THEN** initialization succeeds
- **AND** results stored by runs before it are still served

### Requirement: A caller may run without the cache

A caller SHALL be able to run a chain that neither reads from nor writes to the cache, deciding this
per run rather than for the application as a whole.

Such a run SHALL compute every operation, SHALL return the same result it would have returned with
the cache in force, and SHALL leave nothing behind: a caller that must not persist the images it
processes has to be able to rely on that, and so does one measuring what a model costs, which would
otherwise measure a read or charge the storing of a result to the model.

Reading and writing SHALL be decided once for a run rather than per operation, so that a run cannot
read from the cache and then decline to write back to it.

#### Scenario: A run with the cache turned off

- **WHEN** a chain is run with the cache turned off, whose result is already stored
- **THEN** every operation runs
- **AND** the result matches what a cached run would have produced

#### Scenario: What a run with the cache turned off leaves behind

- **WHEN** a chain is run with the cache turned off
- **THEN** nothing it computed is stored
- **AND** a later cached run of the same chain finds nothing to serve

### Requirement: The store holds results that are not pictures, under their own encoding

The store SHALL hold the results of runs whose result is not a picture beside the pictures an enhancement produces,
each written in the encoding its own kind of value requires.

A second kind of value in one store is only safe if the two can never be mistaken for one another. An entry holding
a measurement of an image SHALL NOT be readable as a picture, and an entry holding a picture SHALL NOT be readable
as a measurement: an entry read back as the wrong kind is a wrong answer rather than a miss, and a wrong answer here
is invisible — a caller handed a plausible value has nothing to compare it against.

Everything the store already guarantees SHALL apply to these entries unchanged: the same lifetime, the same
reclamation, the same backing modes, and the same rule that no failure of the store fails the run that used it.

#### Scenario: A result that is not a picture is stored and read back

- **WHEN** a run whose result is not a picture stores its result and the same run is made again
- **THEN** the stored result is read back and is what the first run produced

#### Scenario: An entry of one kind read as the other

- **WHEN** an entry holding one kind of value is read as the other kind
- **THEN** it is reported as unreadable rather than decoded into a value of the wrong kind

#### Scenario: A result that is not a picture outlives nothing longer than a picture does

- **WHEN** an entry holding a result that is not a picture has been stored for longer than the store keeps entries
- **THEN** it is no longer served, and its space is reclaimed on the same terms a picture's is
