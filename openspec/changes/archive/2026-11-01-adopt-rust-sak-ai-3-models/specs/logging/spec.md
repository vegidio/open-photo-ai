## ADDED Requirements

### Requirement: A run using installed models without the published listing says so

Where the application cannot read the live listing of published models and uses an installed model as it is, that
SHALL be recorded.

A run using an installed model without the listing behaves identically to one with it, right up to the point where
the model was republished and this machine is still running the previous one. The log is where that is
distinguishable.

#### Scenario: The live source is unreachable

- **WHEN** the live listing cannot be read and an installed model is used as it is
- **THEN** a record at warning severity says the live listing failed and that the installed files are used without
  checking for a newer version
- **AND** that record is written once per run, not once per model

#### Scenario: The live source is used

- **WHEN** the live listing is read successfully at raised verbosity
- **THEN** a record says so and how many entries it named

## REMOVED Requirements

### Requirement: A run working from a fallback source says so

**Reason**: The cached copy of the listing and the listing compiled into the build are removed (see the
`model-install` delta), so the fallbacks this requirement distinguished no longer exist.

**Migration**: "A run using installed models without the published listing says so" covers the one fallback that
remains.
