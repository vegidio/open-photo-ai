## Purpose

Decides, once for every front end, what an exported photograph is called, where it goes, which format it is written
in, and how its name is claimed so an existing file is replaced only when the user allowed it and never left
half-written.

## ADDED Requirements

### Requirement: The format choice decides the format and extension written

A naming SHALL carry a format choice that is either one of the eight writable formats (AVIF, BMP, GIF, HEIC, JPEG,
PNG, TIFF, WebP) or Preserve.

A chosen format SHALL be written with its own extension: `jpg` for JPEG, `heic` for HEIC, and the format's own name
in lower case for the rest, whatever the source is.

Under Preserve, a source whose extension names a writable format SHALL be written in that format, keeping the
source's extension in lower case, aliases included (`jpeg`, `tif`, `heif`). A source whose extension names no writable
format, including camera RAW and a file with no extension, SHALL be written as TIFF with the extension `tiff`.

HEIC SHALL be written as HEIF.

#### Scenario: A chosen format over any source

- **WHEN** `/photos/shot.nef` is named with the format JPEG
- **THEN** it is written as JPEG with the extension `jpg`

#### Scenario: Preserve keeps an alias, lower-cased

- **WHEN** `/photos/beach.JPEG` is named under Preserve
- **THEN** it is written as JPEG with the extension `jpeg`

#### Scenario: Preserve over a RAW source

- **WHEN** `/photos/shot.nef` is named under Preserve
- **THEN** it is written as TIFF with the extension `tiff`

#### Scenario: Preserve over a source with no extension

- **WHEN** `/photos/shot` is named under Preserve
- **THEN** it is written as TIFF with the extension `tiff`

### Requirement: The name is the prefix, the source's name, the suffix and the extension

The name written SHALL be the prefix, then the source's file name without its last extension, then the suffix, then
a dot and the extension the format choice decides. Only the last extension SHALL be removed. A name with other dots
in it SHALL keep them.

#### Scenario: Affixes around the name

- **WHEN** `/photos/beach.jpg` is named with the prefix `new-`, the suffix `-opai` and the format WebP
- **THEN** the name is `new-beach-opai.webp`

#### Scenario: A name with dots in it

- **WHEN** `/photos/2026.09.24 beach.jpg` is named under Preserve with no prefix or suffix
- **THEN** the name is `2026.09.24 beach.jpg`

### Requirement: The file goes into the chosen folder, or beside its source

When a folder is chosen, the destination SHALL be the name inside that folder. When none is chosen, it SHALL be the
name inside the directory the source is in, a source at the root of a drive included.

#### Scenario: Beside the source

- **WHEN** `/photos/beach.jpg` is named as PNG with no folder chosen
- **THEN** the destination is `/photos/beach.png`

#### Scenario: A chosen folder

- **WHEN** `/photos/beach.jpg` is named as PNG with `/exports` chosen
- **THEN** the destination is `/exports/beach.png`

#### Scenario: A source at the root

- **WHEN** `/beach.jpg` is named as PNG with no folder chosen
- **THEN** the destination is `/beach.png`

### Requirement: Without overwriting, a free name is claimed and nothing is replaced

When overwriting is not allowed, the destination SHALL be claimed if it is free. Otherwise the first free name formed
by adding `_1`, `_2` and so on, up to `_999`, before the extension SHALL be claimed. A name without an extension SHALL
be numbered at its end. The name claimed SHALL be held from the moment it is claimed, so two writers whose
destinations are the same can never both claim it, even at the same moment. A destination that is the source's own
file SHALL be treated like any other existing file.

When every name up to `_999` is taken, the claim SHALL be refused, naming the destination, and no file SHALL be
replaced. A destination that cannot be written for any other reason, such as a missing or read-only directory, SHALL
be refused with the filesystem's own reason at the first name tried, and nothing SHALL be created.

#### Scenario: The destination is taken

- **WHEN** `photo-opai.png` is claimed without overwriting and that file and `photo-opai_1.png` exist
- **THEN** `photo-opai_2.png` is claimed, and the two existing files are unchanged

#### Scenario: A name with no extension is taken

- **WHEN** `photo` is claimed without overwriting and that file exists
- **THEN** `photo_1` is claimed

#### Scenario: Two writers race for one name

- **WHEN** two claims without overwriting run at the same time for the same free destination
- **THEN** one holds the destination and the other holds its `_1` name

#### Scenario: Every number taken

- **WHEN** a destination and every numbered name up to `_999` beside it exist
- **THEN** the claim is refused naming the destination, and no file is changed

#### Scenario: A directory that does not exist

- **WHEN** a destination inside a directory that does not exist is claimed
- **THEN** the claim is refused as not found, and nothing is created

### Requirement: With overwriting, the destination is replaced only once the file is complete

When overwriting is allowed, the file SHALL be written beside the destination under a hidden name in the same
directory, and SHALL replace the destination only when the write is committed. Until then the destination SHALL be
left exactly as it was, the source's own file included. A replaced file's permissions SHALL be kept. After a commit
no hidden or numbered file SHALL remain.

#### Scenario: Overwriting an existing file

- **WHEN** a destination that exists is claimed with overwriting, written and committed
- **THEN** the destination holds the new file, and the directory holds no other new file

#### Scenario: Not replaced before the commit

- **WHEN** a destination that exists is claimed with overwriting and written, but not yet committed
- **THEN** the destination still holds its earlier contents

#### Scenario: Permissions kept

- **WHEN** a file readable by its owner and group only is overwritten
- **THEN** the file written in its place has the same permissions

### Requirement: A write that does not finish leaves nothing behind

A claim that is given up without being committed SHALL remove the name it held: a failed encode, a stop and a panic
included. Overwriting, the destination SHALL be left as it was.

#### Scenario: Given up without overwriting

- **WHEN** a free destination is claimed and then given up
- **THEN** nothing exists at the destination

#### Scenario: A failed overwrite

- **WHEN** an existing destination is claimed with overwriting, half written, and given up
- **THEN** the destination holds its earlier contents, and nothing else is left beside it
