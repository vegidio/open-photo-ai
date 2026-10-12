## MODIFIED Requirements

### Requirement: The user can decline anonymous usage analytics

The settings surface SHALL carry a choice about whether the application may send anonymous usage analytics, and SHALL
remember it. It SHALL default to on.

The application SHALL honour the choice. While it is off, the application SHALL send nothing to the collector:
- **A launch that begins with it off** SHALL send nothing, from its first record to its last.
- **Turning it off and saving** SHALL stop sending at once. Nothing emitted after the save SHALL be sent, although
  what was already sent stays sent.
- **Turning it on and saving** SHALL take effect from the next launch. The session in which it was turned on SHALL
  NOT resume sending.

Turning the choice off SHALL itself not be reported anywhere. An opt-out that is transmitted is not an opt-out.

The choice the user sees in the settings surface SHALL be the one honoured. If the application finds that its record
of the choice disagrees with what the settings surface shows, the settings surface's value SHALL win, and turning it
off SHALL still stop sending at once.

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off and saves
- **THEN** the choice is remembered across restarts
- **AND** nothing is sent as a result of turning it off
- **AND** nothing the application does for the rest of the session is sent

#### Scenario: A launch after declining

- **WHEN** the user has turned analytics off and then starts the application
- **THEN** the collector receives nothing from that session

#### Scenario: Turning analytics back on

- **WHEN** the user who had turned analytics off turns it on and saves
- **THEN** nothing further is sent during that session
- **AND** the next launch sends as a launch with analytics on does

#### Scenario: A first launch

- **WHEN** the application is started for the first time, in a build that names a collector
- **THEN** it sends, because the choice defaults to on
