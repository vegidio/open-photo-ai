## MODIFIED Requirements

### Requirement: The user can decline anonymous usage analytics

The settings surface SHALL carry a choice about whether the application may send anonymous usage analytics, and SHALL
remember it. It SHALL default to on.

The application SHALL honour the choice. It governs everything the application sends: the backend's records, spans
and metrics, and the window's usage events, failures and session. While it is off, the application SHALL send
nothing to any collector:
- **A launch that begins with it off** SHALL send nothing, from its first record to its last, from the backend or the
  window.
- **Turning it off and saving** SHALL stop sending at once, from both. Nothing emitted after the save SHALL be sent,
  although what was already sent stays sent.
- **Turning it on and saving** SHALL take effect from the next launch, for both. The session in which it was turned on
  SHALL NOT resume sending from either.

Turning the choice off SHALL itself not be reported anywhere. An opt-out that is transmitted is not an opt-out.

The choice the user sees in the settings surface SHALL be the one honoured. If the application finds that its record
of the choice disagrees with what the settings surface shows, the settings surface's value SHALL win, and turning it
off SHALL still stop sending at once.

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off and saves
- **THEN** the choice is remembered across restarts
- **AND** nothing is sent as a result of turning it off
- **AND** nothing the application or its window does for the rest of the session is sent

#### Scenario: A launch after declining

- **WHEN** the user has turned analytics off and then starts the application
- **THEN** no collector receives anything from that session, from the backend or the window

#### Scenario: Turning analytics back on

- **WHEN** the user who had turned analytics off turns it on and saves
- **THEN** nothing further is sent during that session, from the backend or the window
- **AND** the next launch sends as a launch with analytics on does

#### Scenario: A first launch

- **WHEN** the application is started for the first time, in a build that names a collector
- **THEN** it sends, because the choice defaults to on

#### Scenario: The window reloads after declining

- **WHEN** the user turns analytics off and the window is then reloaded in the same session
- **THEN** the reloaded window sends nothing
