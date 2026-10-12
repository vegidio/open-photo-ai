# gui-analytics Specification

## Purpose

Defines what the graphical application's window sends to Grafana while anonymous usage analytics is on: its session,
its failures, and a fixed set of usage events. It also defines what the window never sends.

## Requirements

### Requirement: The window sends only while analytics is on and its build names a collector

The window SHALL send to Grafana only in a launch that began with analytics on, and only in a build that names a
collector for the window. It SHALL decide this from the choice it holds and the build it is, without asking the
application's backend, and SHALL send nothing:
- when the launch began with analytics off
- after analytics was turned off in this session
- in a build that names no collector for the window

The window and the backend SHALL each send only what is their own. Neither SHALL require the other to be sending.

#### Scenario: A launch with analytics on

- **WHEN** the application starts with analytics on, in a build that names the window's collector
- **THEN** the window's session reaches Grafana

#### Scenario: A build without the window's collector

- **WHEN** the application starts with analytics on, in a build that names the backend's collector but not the
  window's
- **THEN** the backend sends and the window sends nothing, and neither is treated as a failure

#### Scenario: A build without the backend's collector

- **WHEN** the application starts with analytics on, in a build that names the window's collector but not the
  backend's
- **THEN** the window sends and the backend sends nothing, and neither is treated as a failure

#### Scenario: Turning analytics off

- **WHEN** the user turns analytics off and saves
- **THEN** the window stops sending before the backend is told of the choice
- **AND** nothing the window held unsent at that moment is sent

### Requirement: The window's failures reach Grafana once

Each failure the window records in the file SHALL, while the window is sending, reach Grafana once, as an exception
carrying the error's text and stack and, for a drawing error, the drawn component path. It SHALL follow the same
rules as the file's record:
- A failure the backend already recorded, or a stop, SHALL NOT be sent.
- The same failure SHALL be sent once per load of the window.
- No more failures SHALL be sent per load than are recorded in the file.

The window SHALL NOT send its console output, and SHALL NOT capture errors by any path other than the one that
records them in the file.

#### Scenario: A rejection nothing handled

- **WHEN** a promise in the window rejects with an error and nothing handles it, while the window is sending
- **THEN** Grafana receives one exception for it
- **AND** the file holds one record of it
- **AND** the backend's collector receives no record of it

#### Scenario: A failure the library already recorded

- **WHEN** an enhancement the window asked for fails while the window is sending
- **THEN** the window sends no exception for it

### Requirement: The window sends a fixed set of usage events

While it is sending, the window SHALL send an event for each of these user actions, and for no others:

| Event | When | Carries |
|---|---|---|
| `app_ready` | setup completes, once per load | the chosen processor, language, background, and whether Autopilot is on |
| `files_added` | images are admitted | how many, how they were brought in (browsing, the empty canvas, or a drop), and their file types |
| `files_refused` | a drop held files that cannot be opened | how many, and their file types |
| `enhancement_added` | an enhancement is added to an image | its family, and whether the user or Autopilot added it |
| `enhancement_removed` | the user removes an enhancement | its family |
| `autopilot_run` | an analysis the user started completes | how many enhancements it suggested, zero included |
| `preview_mode_changed` | the comparison view changes | the view |
| `crop_applied` | a framing that changes something is applied | whether it rotates, flips, or keeps a ratio |
| `export_started` | a batch export starts | how many files, the format, the processor |
| `export_finished` | a batch export ends | how many files, how many were exported, whether it ran to the end, how long it took |
| `settings_saved` | settings that changed something are saved | which preferences changed |
| `tensorrt_prompt_answered` | the TensorRT question is answered | whether it was accepted |
| `update_opened` | the update notice is followed | nothing |

Turning analytics off SHALL NOT be an event.

#### Scenario: Adding enhancements through Autopilot

- **WHEN** the user runs Autopilot on an image and it suggests three enhancements
- **THEN** one `autopilot_run` event carries three
- **AND** three `enhancement_added` events each say Autopilot added them

#### Scenario: An export's own analysis

- **WHEN** an export analyses an image to decide its enhancements
- **THEN** no `autopilot_run` and no `enhancement_added` event is sent for it

#### Scenario: A save that changes nothing

- **WHEN** the user saves settings without changing any preference
- **THEN** no `settings_saved` event is sent

#### Scenario: An aborted export

- **WHEN** the user aborts a batch export after two of five files
- **THEN** one `export_finished` event says five files, two exported, and not run to the end

### Requirement: Nothing the window sends identifies a person or their files

No event SHALL carry a file name, a path, a folder, an image's dimensions, text the user typed, or an error message.
File types SHALL be sent as a sorted list of distinct lowercase extensions.
Every other value an event carries SHALL be a count, a duration, a yes-or-no, or one of the fixed names the
application already uses, such as an enhancement family or a processor.

#### Scenario: Opening files of several types

- **WHEN** the user opens `Holiday.JPG`, `scan.tiff` and `IMG_2.jpg`
- **THEN** the `files_added` event carries three, and the types `jpg,tiff`
- **AND** carries no file name

### Requirement: Each request the window makes is one trace with the backend's work

While it is sending, the window SHALL send a span for each request it makes of the application's backend, named
after the request, lasting from when it was made until it was answered. A request that was refused SHALL have its span
marked as failed. The backend's span for that request, and all the work beneath it, SHALL belong to the window's
trace, as children of the window's span.

Two requests SHALL have no span and SHALL name no trace:
- the one that records the analytics choice, which is never reported
- the one that writes the window's own records to the file, which is not a request for work

A request's span SHALL carry nothing beyond its name, its timing, whether it failed, and what every span the window
sends carries. It SHALL NOT carry the request's arguments or its answer, and no text from an error.

While the window is not sending, its requests SHALL name no trace. The backend SHALL then root the request's trace at
its own span, as it does for a window that sends nothing.

#### Scenario: An enhancement asked for by the window

- **WHEN** the user enhances an image while the window and the backend are both sending
- **THEN** Grafana holds one trace for it
- **AND** its root is the window's span for the `enhance` request
- **AND** the backend's span for that request, and the enhancement's operations beneath it, are in that trace

#### Scenario: A request the backend refuses

- **WHEN** the window asks for an enhancement of an image the backend no longer holds, while both halves are sending
- **THEN** the window's span for that request is marked as failed
- **AND** the backend's span for it, in the same trace, is marked as failed with the reason

#### Scenario: A launch where only the window sends

- **WHEN** the window asks for an enhancement in a build that names the window's collector but not the backend's
- **THEN** Grafana holds the window's span for the request, and nothing from the backend

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off and saves
- **THEN** no span is sent for the request that records the choice
- **AND** no later request the window makes in that session names a trace

#### Scenario: A request carries no user content

- **WHEN** the window asks to reveal an exported file in the file manager, while it is sending
- **THEN** the request's span names the request
- **AND** carries neither the file's path nor any other argument
