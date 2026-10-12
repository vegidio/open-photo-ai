## MODIFIED Requirements

### Requirement: The ONNX Runtime is installed on first initialization

Initialization SHALL take an application name and install the pinned ONNX Runtime for the running
platform under that application's configuration directory, in a `runtime` subdirectory of its own.
The directory is created if it does not exist. Initialization SHALL NOT complete successfully unless
the runtime's files are in place.

The application name determines the configuration directory - `~/.config/<name>` on Linux,
`~/Library/Application Support/<name>` on macOS, `%APPDATA%\<name>` on Windows - so the same name
must be passed on every call for the same installation to be found again.

Initialization SHALL claim the configuration directory for this process before it probes the
machine's hardware, transfers anything or creates anything beneath that directory, and SHALL be
refused where another process of the same user already holds that claim - see the `single-instance`
capability. A refused initialization SHALL leave the configuration directory exactly as it found it
and SHALL transfer nothing.

The application name is validated and the configuration directory resolved before the claim is made,
because the claim is recorded within that directory. A name that cannot be a directory segment is
therefore still rejected as a name rather than as a contended claim.

#### Scenario: First launch on a machine with nothing installed

- **WHEN** initialization runs with an application name and no runtime is present
- **THEN** the pinned archive for the running platform is downloaded, expanded into the `runtime`
  subdirectory of that application's configuration directory, and the archive itself is removed
- **AND** initialization reports success

#### Scenario: An application name that cannot be a directory segment

- **WHEN** initialization runs with a name that is empty, `.`, `..`, or contains a path separator
- **THEN** initialization fails with an error naming the rejected value
- **AND** nothing is created, downloaded or removed on disk

#### Scenario: Another process of the same user is already initialized

- **WHEN** initialization runs with an application name and no runtime is present, while another
  process of the same user is already initialized under that name
- **THEN** initialization fails with an "already running" error
- **AND** no hardware is probed, nothing is downloaded, and nothing is created under the
  configuration directory
