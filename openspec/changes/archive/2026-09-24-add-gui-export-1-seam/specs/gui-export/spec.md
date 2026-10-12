## Purpose

How the window turns an open photograph into a file on disk. This covers the enhancements run over it, the framing
they run on, where the file is written and under what name, what the export reports while it works, and how an export
is stopped or refused.

## ADDED Requirements

### Requirement: The interface can ask for an open photograph to be exported

The application SHALL let its interface ask for an open photograph to be exported. The request names:

- the enhancements to run over it
- the processor to run them on
- the destination to write to
- the format to write
- a quality

The application SHALL run the enhancements over the photograph, in the order given, write the result to the
destination in the named format, and answer the path it actually wrote and the number of bytes written.

The format named SHALL govern the bytes written, whatever the destination's name ends in.

A request with no enhancements SHALL NOT be a failure. It SHALL write the photograph itself, as it is framed, in the
named format.

#### Scenario: A photograph with enhancements

- **WHEN** the interface asks for an open photograph to be exported with an upscale of 2x, as PNG, to a destination
  where no file exists
- **THEN** a PNG twice the photograph's width and height is written at that destination
- **AND** the answer names that destination and the number of bytes written

#### Scenario: A photograph with no enhancements

- **WHEN** the interface asks for an open JPEG photograph to be exported with no enhancements, as WebP
- **THEN** a WebP of the photograph at its own dimensions is written, and the request is not a failure

#### Scenario: The format wins over the destination's name

- **WHEN** the interface asks for an export as JPEG to a destination whose name ends in `.png`
- **THEN** the bytes written are JPEG

### Requirement: An export is made from the photograph as the user has framed it

The enhancements SHALL run over the photograph as it is framed at the time of the request: the crop, rotation and
flip the interface sends are applied first. A request that sends no framing SHALL be exported from the whole
photograph.

#### Scenario: A cropped photograph

- **WHEN** the interface asks for a photograph to be exported with a crop that keeps a 400 by 300 region and no
  enhancements
- **THEN** the file written is 400 by 300

### Requirement: An export keeps as much of the source's depth as the format can hold

The result SHALL carry the source photograph's own bit depth wherever the named format can hold it. It SHALL be
narrowed only where the format forces it.

This is deliberately unlike the enhanced view the window draws, which is always eight bits per channel.

#### Scenario: A 16-bit source written to a 16-bit format

- **WHEN** a 16-bit photograph is exported as PNG
- **THEN** the file written carries 16 bits per channel

#### Scenario: A 16-bit source written to an 8-bit format

- **WHEN** a 16-bit photograph is exported as JPEG
- **THEN** the file written is a valid 8-bit JPEG

### Requirement: The quality applies to the formats that take one

The quality SHALL govern the encoding of AVIF, HEIC, JPEG and WebP. It SHALL have no effect on BMP, GIF, PNG or TIFF.

A quality outside 1 to 100 SHALL be brought inside that range rather than refused.

#### Scenario: Two qualities of one lossy format

- **WHEN** one photograph is exported twice as JPEG, at quality 90 and at quality 30
- **THEN** both files decode to the photograph's dimensions, and the quality 30 file is the smaller

#### Scenario: A quality with a lossless format

- **WHEN** one photograph is exported twice as PNG, at quality 90 and at quality 30
- **THEN** the two files are the same size

### Requirement: Without overwriting, an existing file is never replaced

A request SHALL say whether overwriting is allowed.

When it is not allowed and a file already exists at the destination, the application SHALL write to the first free
name formed by adding `_1`, `_2` and so on, up to `_999`, before the extension. The answer SHALL name the path
actually written.

Two exports whose destinations resolve to the same name SHALL NOT both be written under it, even when they run at
the same time.

When every numbered name up to `_999` is taken, the export SHALL be refused as unwritable, naming the destination,
and no file SHALL be replaced.

A destination that is the photograph's own file SHALL be treated like any other existing file.

#### Scenario: The destination is taken

- **WHEN** a photograph is exported without overwriting to `photo-opai.png`, and that file already exists
- **THEN** the result is written to `photo-opai_1.png`, the existing file is unchanged, and the answer names
  `photo-opai_1.png`

#### Scenario: The destination is the source

- **WHEN** a PNG photograph is exported as PNG, without overwriting, to its own path
- **THEN** the source file is unchanged, and the result is written beside it with `_1` added

#### Scenario: Two exports race for one name

- **WHEN** two exports without overwriting run at the same time and both resolve to the same free destination
- **THEN** one is written to that destination and the other to its `_1` name

### Requirement: With overwriting, the destination is replaced

When overwriting is allowed, the application SHALL write to the destination as named, replacing any file already
there, including the photograph's own file.

#### Scenario: Overwriting an earlier export

- **WHEN** a photograph is exported with overwriting allowed to a destination where an earlier export exists
- **THEN** the destination holds the new result, and no numbered file is created

### Requirement: An export that does not finish leaves no file behind

An export that fails or is stopped before its file is written SHALL leave nothing at the name it would have written,
including a name it had reserved.

#### Scenario: A destination directory that cannot be written

- **WHEN** a photograph is exported to a directory the application may not write to
- **THEN** the export is refused as unwritable, naming the destination, and nothing is created

#### Scenario: Stopped while enhancing

- **WHEN** an export without overwriting is stopped while its enhancements are running
- **THEN** no file exists at the destination or at any numbered name the export tried

### Requirement: Exporting does not disturb the enhanced view

An export SHALL NOT stop the enhancement run the window is drawing, and SHALL NOT release the enhanced result the
window holds. An export's result SHALL NOT be served to the window as an enhanced image.

An enhancement run that the window asks for while an export is working SHALL NOT stop the export.

#### Scenario: An export while the canvas is enhancing

- **WHEN** the window is running an enhancement over one photograph and the interface asks for another photograph to
  be exported
- **THEN** both finish, and the window's enhanced result is served as before

#### Scenario: A new canvas run during an export

- **WHEN** an export is working and the window asks for an enhancement run
- **THEN** the export is not stopped

### Requirement: An export reports what it is doing while it works

An export SHALL report its progress under the name the interface gave it, on a channel of its own rather than the
one enhancement runs report on. So an export never moves the indicator the window draws for its own run.

While the enhancements run, the reports SHALL say, as an enhancement run's do:

- which enhancement is being worked on
- whether its model is being fetched or run, with how far the whole chain has got
- while a model is being fetched, how far the fetch has got

An enhancement whose result the application already had SHALL be reported without either stage.

Once the enhancements are done and the file is being written, the export SHALL report that it is writing. It SHALL do
so whether or not there were enhancements.

#### Scenario: A chain that runs

- **WHEN** an export of a photograph with one enhancement is asked for, and that enhancement's model is on the machine
- **THEN** reports arrive under the export's name saying the enhancement is running, with a fraction that never
  decreases
- **AND** a report that the export is writing follows them
- **AND** no report arrives on the channel enhancement runs report on

#### Scenario: A photograph with no enhancements

- **WHEN** an export with no enhancements is asked for
- **THEN** the only report is that the export is writing

### Requirement: An export the interface no longer wants can be stopped

The interface SHALL name each export it asks for, and SHALL be able to stop an export by that name. A stopped export
SHALL answer that it was stopped, distinct from a failure, and SHALL write no file.

A stop that reaches the application before the export it names has started SHALL still stop that export.

A stop that arrives once the file is being written SHALL let the write finish, and the export SHALL answer as
written.

A stop naming an export that has already finished, or one the application has never heard of, SHALL do nothing and
SHALL NOT be a failure. Stopping one export SHALL NOT affect any other export, any analysis, or the window's
enhancement run.

#### Scenario: Stopped while running

- **WHEN** the interface stops an export while its enhancements are running
- **THEN** the export answers that it was stopped, and no file is written

#### Scenario: A stop that overtakes its export

- **WHEN** the interface stops an export by name before the request to start it has been handled
- **THEN** the export answers that it was stopped without reading the photograph or fetching anything

#### Scenario: A stop for a finished export

- **WHEN** the interface stops an export that has already answered
- **THEN** nothing happens, and the stop is not a failure

### Requirement: An export that cannot be made is refused, and says why

A request SHALL be refused, with each refusal distinguishable from the others:

- **Not ready.** It was made before the application finished starting.
- **Unknown source.** It names an image the user has not opened. This is refused before any file is read or any
  model is fetched.
- **Unknown operation.** It names an enhancement the application does not publish. This is refused before any model
  is fetched for any enhancement in the request, and says which one.
- **Unreadable source.** It names an opened photograph whose file can no longer be read.
- **Enhancement failure.** An enhancement failed while running. This carries the application's own description.
- **Unwritable.** The file could not be written. This names the destination and carries the reason.

None of these refusals SHALL leave a file behind.

#### Scenario: An image that was never opened

- **WHEN** the interface asks for an export naming an identity the application never admitted
- **THEN** the request is refused as an unknown source, and nothing is read, fetched or written

#### Scenario: An unpublished model in the second enhancement

- **WHEN** the interface asks for an export whose second enhancement names a model the application does not publish
- **THEN** the request is refused as an unknown operation naming the second, and no model is fetched for the first

#### Scenario: A file removed after it was opened

- **WHEN** the interface asks for an export of an opened photograph whose file is no longer on disk
- **THEN** the request is refused as an unreadable source

#### Scenario: Before start-up has finished

- **WHEN** the interface asks for an export before the application has finished starting
- **THEN** the request is refused as not ready
