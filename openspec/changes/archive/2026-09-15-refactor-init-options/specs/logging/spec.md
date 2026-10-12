## MODIFIED Requirements

### Requirement: Every record identifies the application that produced it

Three binaries write to one file, and slice 1's claim does not exclude every overlap: a process
refused by it has already started logging by the time it is refused. Every record SHALL therefore
carry the application that produced it, spelled exactly as a single-instance refusal spells it.

The spelling is the identity the process declared rather than one of a fixed set. This project's own
binaries declare `gui`, `cli` and `perf`, and their records SHALL carry those spellings unchanged; an
application embedding this library carries its own. One identity backs both the log field and the
single-instance record, so the two can never disagree about what a process is called.

This SHALL hold for every record in the file, including the ones produced by the inference runtime
rather than by this project's own code.

#### Scenario: A record is written

- **WHEN** any record is written by any of the three applications
- **THEN** it carries the application's name as a field
- **AND** the spelling is the same one a single-instance refusal uses for that application

#### Scenario: A record written by an embedding application

- **WHEN** a record is written by a process that declared its own identity
- **THEN** it carries that identity as the field, spelled as declared

#### Scenario: Two applications in one file

- **WHEN** records from two different applications are present in the file
- **THEN** each record says which application wrote it, independently of which session it belongs to
