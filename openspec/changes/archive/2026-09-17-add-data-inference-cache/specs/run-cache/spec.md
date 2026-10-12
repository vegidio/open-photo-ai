## ADDED Requirements

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
