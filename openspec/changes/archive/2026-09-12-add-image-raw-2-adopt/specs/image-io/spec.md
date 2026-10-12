## ADDED Requirements

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

### Requirement: Bit depth of a developed RAW

A developed camera RAW picture SHALL carry 16 bits per channel, which is the depth the decoder
produces for every RAW file regardless of what the sensor recorded.

Writing a 16-bit picture SHALL be governed by what the target format's encoder accepts. GIF and WebP
accept only 8 bits per channel and SHALL report an error rather than write a corrupt file; the other
supported formats SHALL write it.

This is a known limitation rather than an intended end state: a later change converts a picture down
to a depth the target encoder accepts instead of refusing it.

#### Scenario: A developed RAW is 16-bit
- **WHEN** a camera RAW file is loaded
- **THEN** its pixels carry 16 bits per channel

#### Scenario: A 16-bit picture is refused by the formats that cannot hold it
- **WHEN** a picture carrying 16 bits per channel is written as GIF or as WebP
- **THEN** an error is returned and no valid file is produced

#### Scenario: A 16-bit picture is written by the formats that can hold it
- **WHEN** a picture carrying 16 bits per channel is written as BMP, JPEG, PNG, TIFF, AVIF or HEIF
- **THEN** the file at that destination holds an image in that format

## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Camera RAW files are refused rather than misread

**Reason**: The decoder the requirement was waiting for now exists. `rust-sak`'s `image-raw` feature
shipped in the previous slice, so a RAW file in a format the backend covers is decoded rather than
refused, and a blanket refusal of every RAW extension is no longer the behaviour.

**Migration**: Replaced by three requirements above. "Loading a camera RAW file" keeps everything the
removed requirement protected — extension-first recognition, never decoding a RAW as TIFF, refusal
before the file is read — and applies the refusal only to the recognised RAW formats that have no
decoder. "Recognising camera RAW files" exposes the recognition itself, so a caller can route.
"Describing a camera RAW file without decoding it" covers the metadata side. A caller that treated
the unsupported-format error as meaning "any RAW file" must now expect it only for the formats
without a decoder.
