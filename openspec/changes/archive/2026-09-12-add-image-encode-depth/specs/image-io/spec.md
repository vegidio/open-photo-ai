## ADDED Requirements

### Requirement: Writing a picture a format cannot hold natively

Writing SHALL succeed for every combination of a picture the system can produce and a supported
target format. A picture whose colour layout or sample depth the target encoder cannot accept SHALL
be converted to one it accepts, rather than reported as an unsupported colour type.

The conversion SHALL preserve as much of the original as the target format can hold:

- Sample depth SHALL be narrowed only as far as the encoder forces. A picture written to a format
  that holds 16 bits per channel keeps 16; one written to a format that holds only 8 is narrowed to
  8.
- Transparency SHALL be preserved wherever the target format can hold an alpha channel, and dropped
  only where it cannot.
- Grayscale SHALL be expanded to colour only where the target format has no grayscale
  representation.

A picture the target encoder already accepts SHALL NOT be converted, so that a format capable of
holding the picture exactly writes it exactly.

This requirement governs every format the system writes, not only the formats a camera RAW happens
to produce. It applies equally to a picture loaded from a file: an 8-bit grayscale image is written
by every supported format, as is a 16-bit one.

#### Scenario: Every format writes a 16-bit picture
- **WHEN** a picture carrying 16 bits per channel is written as BMP, GIF, JPEG, PNG, TIFF, AVIF, HEIF or WebP
- **THEN** each destination holds a valid image in that format, and each decodes back to the picture's dimensions

#### Scenario: Every format writes a grayscale picture
- **WHEN** a grayscale picture, with and without an alpha channel, is written in each supported format
- **THEN** each destination holds a valid image in that format

#### Scenario: Precision is kept where the target can hold it
- **WHEN** a 16-bit picture is written as PNG and as TIFF, and the results are described
- **THEN** each reports 16 bits per channel

#### Scenario: Precision is narrowed only where the target forces it
- **WHEN** a 16-bit picture is written as GIF and as WebP, and the results are described
- **THEN** each reports 8 bits per channel, and each decodes to the picture's dimensions

#### Scenario: Transparency survives where the target can hold it
- **WHEN** a 16-bit picture with an alpha channel is written as PNG and as WebP
- **THEN** both results carry an alpha channel

#### Scenario: A picture the format accepts is written unchanged
- **WHEN** an 8-bit picture is written in a lossless format that accepts it natively and read back
- **THEN** its pixels are identical to the pixels that were written

### Requirement: Depth of a developed camera RAW

A developed camera RAW picture SHALL carry 16 bits per channel, which is the depth the decoder
produces for every RAW file regardless of what the sensor recorded.

A developed RAW SHALL be writable in every supported format. Where the target format holds 16 bits
per channel the depth is kept, and where it does not the picture is converted down rather than
refused, as the requirement governing what a format can hold sets out.

#### Scenario: A developed RAW is 16-bit
- **WHEN** a camera RAW file is loaded
- **THEN** its pixels carry 16 bits per channel

#### Scenario: A developed RAW is written in every supported format
- **WHEN** a loaded camera RAW picture is written as BMP, GIF, JPEG, PNG, TIFF, AVIF, HEIF and WebP
- **THEN** each destination holds a valid image in that format at the picture's dimensions

## REMOVED Requirements

### Requirement: Bit depth of a developed RAW

**Reason**: Its two writing scenarios assert the refusal this change removes — that GIF and WebP
report an error for a 16-bit picture — and the requirement named itself a known limitation awaiting
this change. The refusal is gone, so the scenarios that pin it cannot be carried forward.

**Migration**: Replaced by "Depth of a developed camera RAW", which keeps the fact that a developed
RAW carries 16 bits per channel unchanged and replaces the refusal with a write in every supported
format, and by "Writing a picture a format cannot hold natively", which states the general
conversion contract the RAW case is now one instance of. No caller changes: a call that previously
returned an unsupported-colour error now succeeds, and every call that previously succeeded is
unaffected.
