## ADDED Requirements

### Requirement: A photograph whose exposure is wrong calls for light adjustment

The system SHALL read a light signal from the distribution of the image's brightness, and SHALL suggest light
adjustment when that distribution shows the photograph is **underexposed** or **overexposed**:

- **Underexposed:** the whole tonal range is compressed into the shadows, so even the brightest part of the
  photograph never gets bright.
- **Overexposed:** the whole tonal range is lifted off the shadows, so even the darkest part of the photograph never
  gets dark.

The system SHALL NOT suggest light adjustment only because a photograph is dark or bright overall. A night scene or a
low-key portrait that still reaches its highlights is exposed by intent, and so is a high-key scene that still has
shadows. A large area of pure black or pure white is a composition, not evidence of an exposure error.

Where the evidence is borderline, the system SHALL prefer not suggesting. An analysis runs unprompted on a photograph
the user has just opened, and a wrong suggestion there does more harm than a missed one.

The reference implementation suggests light adjustment when the mean brightness is extreme *and* over a third of the
pixels are clipped to near black or near white. That rule is not reproduced. It misses ordinary underexposed
photographs, which are compressed rather than clipped, and it fires on correctly exposed low-key and high-key ones,
which are clipped by intent.

#### Scenario: An underexposed photograph

- **WHEN** a correctly exposed photograph is darkened by two stops and analysed
- **THEN** light adjustment is among the suggestions

#### Scenario: An overexposed photograph

- **WHEN** a correctly exposed photograph is brightened by two stops and analysed
- **THEN** light adjustment is among the suggestions

#### Scenario: A correctly exposed photograph

- **WHEN** a correctly exposed photograph is analysed
- **THEN** light adjustment is not among the suggestions

#### Scenario: A night scene with lights in it

- **WHEN** a photograph that is mostly black but contains bright light sources is analysed
- **THEN** light adjustment is not among the suggestions

#### Scenario: A subject on a white background

- **WHEN** a photograph that is mostly pure white but has a subject with its own shadows is analysed
- **THEN** light adjustment is not among the suggestions

### Requirement: A photograph with a colour cast calls for colour balance

The system SHALL read a colour signal and SHALL suggest colour balance when the photograph's colours are shifted
**as a whole** towards one hue, as estimated from the colour of the light the photograph was taken under.

The system SHALL NOT suggest colour balance only because a photograph is colourful, or because one colour dominates
its content. A forest, a sunset or a red car is not a cast.

The signal SHALL NOT count a pixel that has no hue — equal in all three channels — as belonging to any hue. It SHALL
NOT take its evidence from pixels too dark or too clipped to carry any. Its estimate of the light SHALL move with the
cast being measured, so that a stronger cast moves it further from neutral, never less.

Where the evidence is borderline, the system SHALL prefer not suggesting, for the same reason light adjustment does.

The reference implementation's colour heuristic is not reproduced. It counts pixels with no hue as red, so it flags
greyscale, overexposed and white-background photographs as skewed. Its neutral-pixel test uses a fixed saturation
cutoff, so a strong cast leaves too few pixels to measure and is detected *less* often than a weak one: its evidence
moves the wrong way as the cast grows.

#### Scenario: A warm cast

- **WHEN** a correctly balanced photograph is re-rendered under a tungsten white balance and analysed
- **THEN** colour balance is among the suggestions

#### Scenario: A cool cast

- **WHEN** a correctly balanced photograph is re-rendered under a shade white balance and analysed
- **THEN** colour balance is among the suggestions

#### Scenario: A correctly balanced photograph

- **WHEN** a correctly balanced photograph is analysed
- **THEN** colour balance is not among the suggestions

#### Scenario: A greyscale photograph

- **WHEN** a photograph whose every pixel is equal in all three channels is analysed
- **THEN** colour balance is not among the suggestions

#### Scenario: A stronger cast is not missed where a weaker one is found

- **GIVEN** a correctly balanced photograph that is not suggested colour balance
- **WHEN** it is re-rendered with a cast and that rendering is suggested colour balance
- **THEN** the same photograph re-rendered with a stronger cast of the same hue is suggested colour balance too

### Requirement: The light and colour signals read the pixels and nothing else

The system SHALL read the light and colour signals from the image's own pixels. It SHALL NOT run a model, install
anything or touch the network to read them, and neither signal SHALL be a reason an analysis is reported incomplete.

The two signals SHALL be read from one pass over the image. The pass SHALL cost no more for a large photograph than
for a photograph of a bounded size: the signals are statistics of the whole frame, and they do not need every pixel of
a large one.

A photograph that carries 16 bits per channel SHALL be measured at 16 bits. Transparent pixels SHALL NOT contribute to
either signal: a subject cut out onto a transparent background is not a dark photograph.

A cancelled analysis SHALL still return nothing, whichever signal it was reading when it was cancelled.

#### Scenario: No model is available

- **WHEN** an image is analysed and no model can be installed or opened
- **THEN** the light and colour signals are still read and their suggestions still come back

#### Scenario: A very large photograph

- **WHEN** a photograph of tens of megapixels is analysed
- **THEN** the light and colour signals reach the same conclusions they would reach for the same photograph at a
  bounded size

#### Scenario: A cut-out on a transparent background

- **WHEN** a correctly exposed subject surrounded by fully transparent pixels is analysed
- **THEN** light adjustment is not among the suggestions
