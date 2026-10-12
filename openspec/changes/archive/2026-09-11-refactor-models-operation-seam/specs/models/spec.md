## ADDED Requirements

### Requirement: An operation of any family is one value

The system SHALL provide a single value that can carry an operation of any family the library names,
so that an operation can be stored, serialized, sent across a process boundary and dispatched on
without the holder knowing which family it belongs to.

That value SHALL report, for any family it carries: the family, the precision the operation runs at,
the display name a user interface shows, the artifacts that must be on disk before it can run, and
the image cache tag its result is stored under - each identical to what the family's own type
reports for the same operation.

Carrying an operation in this form SHALL NOT weaken any guarantee the per-family types give. In
particular a variant of one family SHALL NOT be pairable with a different family, and a family that
takes no per-run parameter SHALL NOT gain one.

#### Scenario: Every family can be carried
- **WHEN** an operation of each of the eight families is placed in the shared value
- **THEN** each reports the same family, precision, display name, required artifacts and cache tag as
  the family's own type reports for that same operation

#### Scenario: A codename cannot cross families
- **WHEN** an operation is built for a family
- **THEN** only that family's variants can be supplied, and pairing another family's codename with it
  is not expressible

#### Scenario: A carried operation survives a round trip
- **WHEN** an operation is serialized from the shared value and deserialized back
- **THEN** the result equals the original, including its family, its variant, its precision and any
  per-run parameter it carries

### Requirement: A published catalogue row can be turned back into a runnable operation

The system SHALL provide, as public API, the inverse of the catalogue: given a family, a variant
codename and a precision as the catalogue publishes them, it SHALL produce the operation that
pairing names, or report that the pairing names none.

The inverse SHALL offer one entry point for the families that take no per-run parameter and one that
also takes that parameter for the families that do, and the catalogue itself SHALL say which of the
two a row needs - a family that publishes a parameter range takes the second, and a family that
publishes none takes the first. Neither SHALL accept what the other is for: a family that takes a
parameter SHALL NOT be buildable without one, and a family that takes none SHALL have nowhere to put
one.

Every variant and precision the catalogue publishes SHALL be buildable through the entry point its
family's published parameters call for, and a pairing the catalogue does not publish SHALL report
that it names no operation rather than producing one that resolves to an artifact which was never
published.

This inverse SHALL be available to any consumer of the library, not only to its own tests, so that a
front end populating a chooser from the catalogue can turn the user's selection back into an
operation without restating any model vocabulary of its own.

#### Scenario: Every published row builds
- **WHEN** every variant and precision the catalogue publishes is passed to the entry point that row's
  published parameters call for
- **THEN** each produces an operation, and each such operation reports at least one required artifact

#### Scenario: Each entry point refuses what the other is for
- **WHEN** a family that publishes a parameter range is passed to the entry point that takes none, or a
  family that publishes none is passed to the entry point that takes one
- **THEN** the inverse reports that the pairing names no operation, rather than defaulting a parameter
  in or dropping one that was supplied

#### Scenario: An unpublished pairing names nothing
- **WHEN** a variant is paired with a precision the catalogue does not publish for it - Osaka at FP32,
  or any single-graph variant at INT8
- **THEN** the inverse reports that the pairing names no operation, and no operation is produced

#### Scenario: An unknown codename names nothing
- **WHEN** a codename no family publishes is supplied
- **THEN** the inverse reports that the pairing names no operation

### Requirement: A family added later cannot be silently skipped

The system SHALL name every family it offers in one place, and everything that iterates families -
the catalogue, the cross-family cache-tag checks, and the shared operation value - SHALL be checked
against that one list rather than against a transcribed copy of it, so that a family omitted from any
of them fails to compile or fails a check that names it. Where a list of families cannot be generated
from that one list - the arms of a type, which no list can write - the check SHALL be what holds it,
and a family the type cannot carry SHALL still be a compile error somewhere that reaches every
family.

A family added to the library and omitted from the catalogue SHALL cause a failure that names the
omission, rather than passing because both the expectation and the published list were missing it.

#### Scenario: The catalogue publishes every family the library names
- **WHEN** the set of families the catalogue publishes is compared against the set the library names
- **THEN** the two are equal, and a family present in one and absent from the other fails the check
  naming that family

#### Scenario: Completeness does not rest on a counted total
- **WHEN** a published variant row is removed
- **THEN** the checks fail by naming what is missing, rather than by a total that no longer matches a
  transcribed number
