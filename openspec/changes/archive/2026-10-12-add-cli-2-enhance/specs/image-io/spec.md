## ADDED Requirements

### Requirement: Recommended encoder settings per format

The library SHALL publish, for each format it writes, the encoder settings a front end uses unless the user chooses
otherwise. Every front end then writes a given format the same way.

- **Default quality:** JPEG 90, AVIF 60, HEIF 60, WebP 75. BMP, GIF, PNG and TIFF take no quality.
- **Quality range:** 1 to 100 for the four lossy formats. A requested quality outside it SHALL be brought inside it
  rather than refused. A value that is not a number SHALL be treated as the lowest quality, never as 0.
- **AVIF speed:** 6.
- Every other setting is the encoder's own default.

A caller SHALL be able to ask for these settings with a quality of its own, or with none to take the format's
default. Writing with no settings at all SHALL still use the codec's own defaults, as "Encoder settings" requires.

#### Scenario: Default quality per format

- **WHEN** the recommended settings are requested for JPEG, AVIF, HEIF and WebP with no quality
- **THEN** they carry qualities 90, 60, 60 and 75 respectively

#### Scenario: A lossless format

- **WHEN** the recommended settings are requested for PNG, with or without a quality
- **THEN** they carry no quality and are PNG's encoder defaults

#### Scenario: A quality out of range

- **WHEN** the recommended settings are requested for JPEG at quality 150, and again at quality 0
- **THEN** the first carries quality 100 and the second quality 1

#### Scenario: AVIF speed

- **WHEN** the recommended settings are requested for AVIF
- **THEN** they set speed 6

#### Scenario: The settings target their own format

- **WHEN** an image is written as any format with that format's recommended settings
- **THEN** the write succeeds and is not refused as settings for another format
