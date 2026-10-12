## Purpose

Reads an image file into memory together with the identity that keys the per-operation cache, writes
an image back to disk in a caller-chosen format, and reports an image's dimensions without decoding
it, so that inference, caching and export all work from one description of what an image is.

## ADDED Requirements

### Requirement: Loading an image from a file

The system SHALL decode an image file into memory and report, together with its pixels, the path it
was read from and an identity derived from its bytes.

The encoded format SHALL be determined from the file's content rather than from its name, so that a
correctly formed image is decoded even when its extension is wrong or absent.

A file that cannot be read, or whose content is not a supported encoded image, SHALL be reported as
an error naming the file. No partially decoded image is ever produced.

#### Scenario: A supported image is decoded
- **WHEN** a readable file holding a BMP, GIF, JPEG, PNG, TIFF, AVIF, HEIF or WebP image is loaded
- **THEN** its pixels, its path and its identity are reported together

#### Scenario: The extension does not decide the format
- **WHEN** a file holding JPEG content is loaded under a name ending in `.png`
- **THEN** it is decoded as JPEG

#### Scenario: A file that is not an image is refused
- **WHEN** a readable file whose content matches no supported format is loaded
- **THEN** an error naming the file is returned, and no image is produced

#### Scenario: An unreadable file is refused
- **WHEN** a file that does not exist or cannot be opened is loaded
- **THEN** an error naming the file is returned, and no image is produced

### Requirement: Image identity

A loaded image SHALL carry an XXH3-64 identity computed over the whole of the file's bytes,
expressed as lowercase hexadecimal.

That identity SHALL equal the XXH3-64 of the same file computed independently, so that a caller
holding only a path and a caller holding a loaded image agree on which image they are describing.
This is what lets a file be listed, hashed and later loaded without being decoded twice.

The identity SHALL describe the bytes that were read, and SHALL NOT be silently carried onto
different pixels.

#### Scenario: Identity matches independent hashing of the same file
- **WHEN** a file is loaded and the same file is hashed directly with XXH3-64
- **THEN** the two values are equal

#### Scenario: Identity covers the whole file
- **WHEN** an image whose encoded data is followed by trailing bytes the decoder does not read is loaded
- **THEN** its identity still equals the XXH3-64 of the entire file

#### Scenario: Two identical files share one identity
- **WHEN** two files with identical bytes and different names are loaded
- **THEN** their identities are equal

### Requirement: Dimensions of a loaded image

A loaded image SHALL report its own width and height, and those SHALL always agree with its pixels.

#### Scenario: A loaded image reports its size
- **WHEN** an image of a known width and height is loaded
- **THEN** it reports that width and height

### Requirement: Writing an image to a chosen destination

The system SHALL write an image to a destination supplied by the caller, in a format supplied by the
caller, and SHALL report how many bytes were written.

The destination SHALL be independent of the path the image was loaded from, so that writing a result
never overwrites its source unless the caller names the source as the destination.

The format the caller supplies SHALL govern the bytes written. The destination's name SHALL NOT
change which format is written.

A destination that cannot be written SHALL be reported as an error naming it.

#### Scenario: An image is written in the requested format
- **WHEN** an image is written to a destination in a supported format
- **THEN** the file at that destination holds an image in that format, and the number of bytes written is reported

#### Scenario: The source is untouched
- **WHEN** a loaded image is written to a different destination
- **THEN** the file it was loaded from is unchanged

#### Scenario: The requested format wins over the destination's name
- **WHEN** an image is written as JPEG to a destination whose name ends in `.png`
- **THEN** the bytes written are JPEG

#### Scenario: An unwritable destination is refused
- **WHEN** the destination is in a directory that does not exist or cannot be written
- **THEN** an error naming the destination is returned

### Requirement: Encoder settings

The caller SHALL be able to write an image either with the encoder's default settings or with
settings of its own, and settings SHALL be expressed per format so that a value only has meaning
against the encoder that accepts it.

Settings that target a format other than the one being written SHALL be refused rather than ignored.

#### Scenario: Defaults are used when no settings are given
- **WHEN** an image is written without settings
- **THEN** it is encoded with the format's default settings

#### Scenario: Supplied settings are applied
- **WHEN** an image is written as JPEG at two different qualities
- **THEN** both files decode to that image, and the lower quality produces the smaller file

#### Scenario: Settings for the wrong format are refused
- **WHEN** an image is written as JPEG with settings that target PNG
- **THEN** an error is returned and nothing is written

### Requirement: Reading dimensions without decoding

The system SHALL report an image file's format, width, height, colour type and bit depth without
decoding its pixels, so that a large file can be described without the cost of decoding it.

Bit depth SHALL be the true per-channel depth, so that high-bit-depth AVIF and HEIF are
distinguishable from 16-bit images.

Unlike loading, probing SHALL take the encoded format from the file's name rather than from its
content. The asymmetry is about cost: loading already holds every byte, because the identity is
computed over them, so reading the format from the content is free there, while probing exists
precisely to avoid holding those bytes.

A file whose name names no supported format SHALL therefore be reported as an error, even where
loading the same file would decode it. What a mis-named file loses is its description; it remains
loadable and writable.

A file that cannot be read, or whose header cannot be parsed, SHALL be reported as an error naming
the file.

#### Scenario: A file is described without being decoded
- **WHEN** an image file is probed
- **THEN** its format, width, height, colour type and bit depth are reported

#### Scenario: Probing agrees with loading
- **WHEN** a file whose name names its format is probed and loaded
- **THEN** the width and height reported by each are equal

#### Scenario: A name that names no supported format is refused
- **WHEN** a file holding a supported image is probed under a name matching no supported format
- **THEN** an error naming the file is returned, and loading that same file still decodes it

#### Scenario: An unreadable file is refused
- **WHEN** a file that does not exist is probed
- **THEN** an error naming the file is returned

### Requirement: Camera RAW files are refused rather than misread

Most camera RAW files carry the same leading bytes as a plain TIFF, so content-based routing would
decode them as TIFF and yield an embedded preview or corrupt pixels instead of the photograph.

The system SHALL recognise camera RAW files by their extension, before any content-based routing,
and SHALL report them as an unsupported format naming the extension. It SHALL NOT decode them as
TIFF or as any other format.

This is a deliberate, temporary gap against the Go implementation, which decodes RAW. A later change
supplies the decoder behind this same detection.

#### Scenario: A RAW file is refused by extension
- **WHEN** a file whose extension is a camera RAW format is loaded
- **THEN** an error reporting an unsupported format, naming the extension, is returned

#### Scenario: A RAW file is never decoded as TIFF
- **WHEN** a RAW file whose leading bytes are those of a TIFF is loaded
- **THEN** it is refused, and no image is produced

#### Scenario: A genuine TIFF still loads
- **WHEN** a real TIFF file is loaded
- **THEN** it is decoded normally

### Requirement: Blocking and non-blocking forms

Every operation in this capability SHALL be available both in a form that may be called from an
asynchronous context without occupying the thread that drives it, and in a form that may be called
where no asynchronous runtime exists.

Both forms of an operation SHALL produce the same result for the same input, and SHALL report the
same errors.

If the asynchronous runtime is shutting down before the work can run, that SHALL be reported as
cancellation and distinguishable from a failure to read or write.

#### Scenario: Both forms agree
- **WHEN** the same file is loaded through the asynchronous form and through the blocking form
- **THEN** both report the same pixels, the same path and the same identity

#### Scenario: The blocking form needs no runtime
- **WHEN** the blocking form is called with no asynchronous runtime present
- **THEN** it completes normally

#### Scenario: Shutdown is reported as cancellation
- **WHEN** the asynchronous form is called and the runtime shuts down before the work runs
- **THEN** cancellation is reported, distinct from a read or write failure
