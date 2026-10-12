# gui-export Specification

## Purpose

How the window turns open photographs into files on disk. This covers the enhancements run over each, the framing
they run on, where each file is written and under what name, what an export reports while it works, and how an export
is stopped or refused. It also covers the export dialog: which picked images it queues and in what order, the
settings it offers, how it runs the queue one file at a time and reports each file's stage, and how a written file is
shown in the file manager.

## Requirements

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

### Requirement: The export control opens the export dialog for the picked images

The sidebar's export control SHALL be available while at least one open image is picked, and SHALL be shown as
unavailable otherwise. It SHALL read "Export image" while one image is picked and "Export images" while several are.
Using it SHALL open the export dialog over the window.

Which image is current SHALL NOT decide availability: the control acts on the picked images.

#### Scenario: Nothing picked

- **WHEN** images are open and none of them is picked
- **THEN** the export control is shown and unavailable

#### Scenario: One image picked

- **WHEN** exactly one open image is picked
- **THEN** the export control is available and reads "Export image"

#### Scenario: Several images picked

- **WHEN** three open images are picked
- **THEN** the export control is available and reads "Export images"
- **AND** using it opens the export dialog

### Requirement: Every picked image is queued, in pipeline order

The export dialog SHALL list one row for each image that was picked when the dialog opened, and no other. What an
image's stack holds SHALL NOT keep it out of the queue. An image with no enhancements is exported as it is framed.

The queue SHALL be ordered so that images whose stacks run the same enhancements, in the same order, are next to each
other. Groups SHALL keep the drawer order of their first image. Within a group, images SHALL keep the order of the
drawer. Images with no enhancements yet form one group. The queue SHALL say that it is shown in pipeline
order.

The queue SHALL NOT change while the dialog is open. An image picked or unpicked behind the dialog SHALL NOT be
added or removed.

#### Scenario: Mixed stacks

- **WHEN** four images are picked in drawer order A, B, C and D, where A and C carry an upscale, and B and D carry a
  denoise followed by an upscale
- **THEN** the queue lists A, C, B, D

#### Scenario: An image with nothing done to it

- **WHEN** an image with no enhancements is picked with Autopilot off, and the user exports
- **THEN** it is in the queue, and the file written is the photograph as framed, at its own dimensions

### Requirement: Each queued row describes the file it will write

Each row SHALL show:

- a thumbnail of the image as framed
- the file name that will be written
- the dimensions as framed, and the dimensions after its enhancements
- the source file's size, and, once its file is written, the size written
- the source's type, and the type that will be written
- its stage

The name, dimensions and types SHALL follow the export settings and the image's stack as they change while the batch
is idle.

The name written SHALL be the prefix, the source's name without its extension, the suffix, and the extension of
the format written. The extension SHALL be:

- **Preserve:** the source's own extension. A source whose format cannot be written, such as a camera RAW file, is
  written as TIFF with the `tiff` extension.
- **JPEG:** `jpg`.
- **HEIC:** `heic`.
- **Any other format:** its own name, in lower case.

The file SHALL be written into the chosen folder. Where the folder is the original directory, it SHALL be written
into the directory the source is in.

#### Scenario: Affixes and a format

- **WHEN** the prefix is `new-`, the suffix is `-opai`, the format is WebP, and the queue holds `/photos/beach.jpg`
  with a 2x upscale over a 1000 by 800 source, framed whole
- **THEN** its row names `new-beach-opai.webp`, shows 1000 x 800 becoming 2000 x 1600, and shows JPG becoming WEBP
- **AND** the file is written as `/photos/new-beach-opai.webp`

#### Scenario: Preserve over a RAW source

- **WHEN** the format is Preserve and the queue holds `shot.nef`
- **THEN** its row names `shot.tiff` and shows NEF becoming TIFF

#### Scenario: A browsed folder

- **WHEN** the user has chosen `/exports` as the folder, and the queue holds `/photos/beach.jpg`
- **THEN** the file is written into `/exports`

### Requirement: The export settings decide each file's name, folder, format and quality

The dialog SHALL offer these settings:

- **Filename:** a prefix and a suffix.
- **Save to:** the original directory, or a folder the user browses for with the platform's own folder picker.
  - Once browsed, the folder's path SHALL be shown as the value.
  - Dismissing the picker SHALL leave the previous choice in force.
- **Allow overwriting:** a switch.
  - While it is off, a footnote SHALL say that new files get a number instead of overwriting the original.
  - While it is on, a warning-coloured footnote SHALL say that the original file will be overwritten and that this
    cannot be undone.
- **Format:** Preserve, AVIF, BMP, GIF, HEIC, JPG, PNG, TIFF or WEBP.
- **Quality:** a slider from 1 to 100. It SHALL be shown only while every queued image will be written in the same
  one of the four lossy formats: AVIF, HEIC, JPEG and WebP. It SHALL start at that format's quality from Settings.

The prefix, suffix, folder, overwriting and format SHALL be remembered across launches. Before the user has chosen
anything, they SHALL be:

- no prefix and no suffix
- the original directory
- overwriting off
- PNG

The quality SHALL be a draft until the user saves. Saving SHALL commit it to Settings as that format's quality.
Closing the dialog without saving SHALL discard it.

Where the slider is hidden, each file SHALL be written at its own format's quality from Settings. A lossless format
takes no quality.

#### Scenario: Mixed formats under Preserve

- **WHEN** the format is Preserve and the queue holds a JPEG and a PNG
- **THEN** no quality slider is shown
- **AND** saving writes the JPEG at the JPEG quality from Settings

#### Scenario: One lossy format

- **WHEN** the format is WEBP
- **THEN** the quality slider is shown, at the WebP quality from Settings

#### Scenario: A draft discarded

- **WHEN** the user moves the quality slider and then cancels
- **THEN** the quality in Settings is unchanged

#### Scenario: A draft committed

- **WHEN** the user moves the WebP quality to 70 and saves
- **THEN** the WebP quality in Settings reads 70, and every file in the batch is written at 70

#### Scenario: Settings remembered

- **WHEN** the user sets the suffix to `-opai` and the format to JPG, closes the dialog and restarts the application
- **THEN** the dialog opens with the suffix `-opai` and the format JPG

#### Scenario: Overwriting switched on

- **WHEN** the user switches Allow overwriting on
- **THEN** the footnote is replaced by the warning-coloured one

### Requirement: Saving runs the queue one file at a time and reports each file's stage

Saving SHALL export the queued images one after another, in queue order. Each row SHALL show one stage:

- **Queued:** not reached yet.
- **Analysing:** the image is being analysed before it is enhanced, by Autopilot or to find the faces a face
  recovery needs.
- **Enhancing:** its enhancements are running.
- **Writing:** the file is being encoded and written.
- **Done:** the file is written.
- **Failed:** it could not be exported.

Only the row being worked on SHALL show a bar of its own. The bar SHALL follow its enhancements' progress.

One overall bar SHALL carry the batch: finished rows, plus the fraction of the row in progress, over the number of
rows. The dialog's title SHALL summarise the batch:

- **Before saving:** how many files are ready, and that nothing is written yet.
- **While running:** how many are written out of how many, and how many remain after the one in progress.
- **Once ended:** how many were written out of how many, and how many failed, if any did.

A face recovery SHALL restore the faces the user has left chosen for that image at its framing. Where those faces
are not known yet, they SHALL be found before the image is enhanced.

Each file SHALL be exported on the processor chosen in Settings, at the image's framing.

#### Scenario: A batch in progress

- **WHEN** the second of four rows is being enhanced and the first is written
- **THEN** the first reads Done with its written size, the second reads Enhancing with its own bar, the other two
  read Queued
- **AND** the title reads "1 of 4 written · 2 remaining"

#### Scenario: A batch finished

- **WHEN** every one of four rows has been written
- **THEN** every row reads Done, the overall bar is full, and the title reads "4 of 4 written"

#### Scenario: A face recovery at export

- **WHEN** a queued image carries a face recovery whose faces have not been found at its framing
- **THEN** its row reads Analysing while the faces are found, and then Enhancing

### Requirement: A file that fails does not stop the batch

A file that cannot be exported SHALL end its row as Failed. The batch SHALL go on to the next row. When the batch
ends, the title SHALL count the failed files separately from those written.

Hovering the Failed stage SHALL show why the file failed, in a red tooltip. For a file that could not be written, the
reason SHALL name the file and the folder, followed by the operating system's own explanation, untranslated.
Clicking the Failed stage SHALL copy that reason to the clipboard, and the tooltip SHALL then confirm the copy.

#### Scenario: One file among four fails

- **WHEN** the second of four rows cannot be written because its folder refuses the write
- **THEN** it reads Failed, the other three read Done, and the title reads "3 of 4 written, 1 failed"

#### Scenario: Copying the reason

- **WHEN** the user clicks a Failed stage
- **THEN** the reason shown in its tooltip is on the clipboard, and the tooltip says it was copied

### Requirement: The dialog cannot be dismissed while the batch runs

While a batch runs, the dialog SHALL show no close box, SHALL NOT close on Escape or on a click outside it, and its
Save button SHALL be unavailable. The Cancel button SHALL read Abort. The export settings SHALL be unavailable, so
the names the rows show are the names being written.

At no time SHALL a click outside the dialog close it.

#### Scenario: Escape while running

- **WHEN** the user presses Escape while a batch runs
- **THEN** the dialog stays open and the batch continues

#### Scenario: Settings while running

- **WHEN** a batch runs
- **THEN** the filename, folder, overwriting, format and quality controls are unavailable

### Requirement: Abort ends the batch

Abort SHALL stop the file in progress, and no further file SHALL be started. The stopped file SHALL leave nothing
on disk. Its row, and every row not reached, SHALL read Queued. Rows already written SHALL keep Done. The batch SHALL
then have ended, as a finished one has.

A file that is already being written when Abort is pressed SHALL finish, and its row SHALL read Done.

#### Scenario: Aborting the third of five

- **WHEN** the user aborts while the third of five rows is being enhanced
- **THEN** the first two read Done, the other three read Queued, no file is written for the third
- **AND** the buttons read Close and Export again

#### Scenario: Aborting while a file is written

- **WHEN** the user aborts while the second of three rows reads Writing
- **THEN** its file is written and its row reads Done, and the third row reads Queued

### Requirement: An ended batch offers to close or to start again

Once a batch has ended, whether it finished, failed in part or was aborted:

- The Cancel button SHALL read Close.
- The Save button SHALL read Export again, drawn as a secondary action.

Export again SHALL reset the queue to its state before saving, every row Queued and nothing written, and SHALL NOT
start a batch by itself.

#### Scenario: Export again

- **WHEN** the user presses Export again after a batch that ended
- **THEN** every row reads Queued, the title says nothing is written yet, and the buttons read Cancel and Save

### Requirement: A written file can be shown in the file manager

A row whose file is written SHALL offer a control that shows that file, selected, in the platform's file manager.
Where the file was written under a numbered name, the numbered file SHALL be the one shown.

The application SHALL show only files an export of this session wrote.

Where the file manager cannot be opened, the user SHALL be told, and told that the file is still saved at its export
location.

#### Scenario: Revealing a numbered file

- **WHEN** a file was written as `beach-opai_1.png` because `beach-opai.png` was taken, and the user reveals it
- **THEN** the file manager opens with `beach-opai_1.png` selected

#### Scenario: A path no export wrote

- **WHEN** the interface asks for a path to be shown that no export of this session wrote
- **THEN** the request is refused and nothing is opened
