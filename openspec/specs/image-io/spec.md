# image-io Specification

## Purpose

Reads an image file into memory together with the identity that keys the per-operation cache, writes
an image back to disk in a caller-chosen format, and reports an image's dimensions without decoding
it, so that inference, caching and export all work from one description of what an image is.

## Requirements

### Requirement: Loading an image from a file

The system SHALL decode an image file into memory and report, together with its pixels, the path it
was read from and an identity derived from its bytes.

For every format other than camera RAW, the encoded format SHALL be determined from the file's
content rather than from its name, so that a correctly formed image is decoded even when its
extension is wrong or absent. Camera RAW files are the exception and are routed by extension, for
the reason their own requirement gives.

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

This operation covers the formats that can also be written, and therefore does not cover camera RAW.
A file whose name names a RAW format SHALL be reported as an error here, and is described through the
separate RAW operation instead.

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

#### Scenario: A RAW name is refused by this operation
- **WHEN** a file whose name names a camera RAW format is probed
- **THEN** an error naming the file is returned, and the RAW operation describes it instead

#### Scenario: An unreadable file is refused
- **WHEN** a file that does not exist is probed
- **THEN** an error naming the file is returned

### Requirement: Loading a camera RAW file

The system SHALL decode a camera RAW file into a developed, display-ready picture, and SHALL report
it with the same path, identity and dimensions as an image in any other format.

Camera RAW files SHALL be recognised by their extension, before any content-based routing. Most RAW
formats are TIFF containers and carry the same leading bytes as a plain TIFF, so routing on content
would decode them as TIFF and yield an embedded preview or corrupt pixels instead of the photograph.

Not every recognised RAW format can be decoded. A recognised RAW extension for which the system has
no decoder SHALL be reported as an unsupported format naming the extension, before the file is read,
so that a RAW file on an unreachable drive reports the format rather than a read failure. It SHALL
NOT be decoded as TIFF or as any other format.

A RAW file whose extension is decodable but whose content cannot be decoded — an unrecognised camera,
or a corrupt file — SHALL be reported as a decode failure naming the file, the same way a corrupt
file in any other format is. A file whose extension names a RAW format but whose content is not RAW
at all SHALL also be reported as a decode failure rather than decoded as whatever it turns out to be.

The identity of a loaded RAW file SHALL be the XXH3-64 of the whole of its bytes, equal to hashing
that file independently, exactly as for every other format.

#### Scenario: A supported RAW file is decoded
- **WHEN** a readable file whose extension names a decodable camera RAW format is loaded
- **THEN** its developed pixels, its path and its identity are reported together

#### Scenario: A RAW format with no decoder is refused
- **WHEN** a file whose extension names a recognised RAW format the system cannot decode is loaded
- **THEN** an error reporting an unsupported format, naming the extension, is returned

#### Scenario: An undecodable RAW format is refused without reading the file
- **WHEN** a path whose extension names a recognised RAW format the system cannot decode is loaded, and that path cannot be read
- **THEN** the unsupported-format error is returned rather than a read failure

#### Scenario: A RAW file is never decoded as TIFF
- **WHEN** a file whose extension names a RAW format and whose leading bytes are those of a TIFF is loaded
- **THEN** it is either decoded as RAW or refused, and never decoded as a TIFF

#### Scenario: A genuine TIFF still loads
- **WHEN** a real TIFF file is loaded
- **THEN** it is decoded normally

#### Scenario: A RAW file that cannot be decoded is reported as a decode failure
- **WHEN** a file whose extension names a decodable RAW format holds content the RAW decoder refuses
- **THEN** an error naming the file, carrying what the decoder reported, is returned, and no image is produced

#### Scenario: A RAW identity is the whole file
- **WHEN** a RAW file is loaded and the same file is hashed directly with XXH3-64
- **THEN** the two values are equal

### Requirement: Recognising camera RAW files

The system SHALL let a caller ask whether a path names a camera RAW file, so that a caller can
choose between the operations that describe RAW files and those that describe the other formats
before calling either.

Recognition SHALL be independent of whether the format can be decoded: a RAW format with no decoder
is still a RAW format, and must never be routed to a non-RAW decoder.

Recognition SHALL ignore the case the camera wrote, because a camera writes `.NEF` as readily as
`.nef` and on a case-sensitive filesystem those are two strings for one format.

#### Scenario: A RAW path is recognised
- **WHEN** a caller asks whether a path whose extension names a camera RAW format is RAW
- **THEN** the answer is yes, whether or not that format can be decoded

#### Scenario: A standard image is not RAW
- **WHEN** a caller asks whether a path ending in a non-RAW supported extension, including `.tif` and `.tiff`, is RAW
- **THEN** the answer is no

#### Scenario: Recognition ignores case
- **WHEN** a caller asks about the same RAW extension in upper, lower and mixed case
- **THEN** the answer is the same for all three

### Requirement: Describing a camera RAW file without decoding it

The system SHALL report a camera RAW file's dimensions, the RAW format it is in, the bit depth the
sensor recorded, the camera's make and model, and whether the container is a DNG, without decoding
its pixels.

This SHALL be a separate operation from describing an image in one of the other supported formats.
The two report different facts — a RAW file has a camera make and model and no colour type — and
they differ in cost: describing a RAW file reads the whole file, because RAW metadata lives at
offsets that routinely point deep into it, where describing another format reads only a header. One
operation covering both would hide that difference from the caller paying it.

The dimensions reported SHALL be the dimensions the file decodes to, so that a listing built from
this operation agrees with the picture that is later loaded.

Sensor bit depth SHALL be reported as absent when the file does not record it, rather than
substituted with a default, and it describes the sensor rather than the decoded picture. Make and
model SHALL be reported as empty when the file does not carry them.

A recognised RAW extension for which the system has no decoder SHALL be reported as an unsupported
format naming the extension. A file that cannot be read SHALL be reported as a read failure, and one
whose metadata cannot be parsed SHALL be reported as an error naming the file, each told apart from
the other as loading tells them apart.

#### Scenario: A RAW file is described without being decoded
- **WHEN** a decodable camera RAW file is described
- **THEN** its RAW format, width, height, sensor bit depth, camera make and model, and whether it is a DNG are reported

#### Scenario: Describing and loading agree about the size
- **WHEN** the same RAW file is described and loaded
- **THEN** the width and height reported by each are equal

#### Scenario: A RAW format with no decoder is refused
- **WHEN** a file whose extension names a recognised RAW format the system cannot decode is described
- **THEN** an error reporting an unsupported format, naming the extension, is returned

#### Scenario: An unreadable RAW file is refused
- **WHEN** a RAW file that does not exist is described
- **THEN** a read failure naming the file is returned, the same failure loading that path reports

#### Scenario: A file that is not RAW is refused
- **WHEN** a file whose extension names a decodable RAW format but whose content is not RAW is described
- **THEN** an error naming the file is returned, and no description is produced

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
