## MODIFIED Requirements

### Requirement: An opened image's pixels are served to the interface at a requested size

The application SHALL serve the pixels of an opened image to the interface, addressed by that
image's identity together with a bound on the image's longest edge, and optionally a crop to apply
before the bound.

A bound SHALL cap the longest edge while preserving the image's aspect ratio. A request carrying no
bound SHALL serve the image at its own dimensions. A bound at or above the image's longest edge
SHALL serve the image at its own dimensions rather than enlarging it.

A request carrying a crop SHALL serve the framed pixels rather than the whole photograph, and the
bound SHALL cap the longest edge of what is served — the framing, not the photograph it was taken
from. A request carrying no crop SHALL be served exactly as it is today: the crop is an addition to
what may be asked for, not a change to what an existing request means.

A bounded request carrying a crop SHALL show the same framing as the unbounded request carrying that
crop, at the smaller size. What is served SHALL be the framing the crop describes, whether or not the
application reduced the photograph first to avoid doing full-resolution work for a thumbnail.

A request whose crop cannot be read SHALL be refused rather than served without it, for the reason a
malformed bound is refused: serving the whole photograph where a framing was asked for is a wrong
answer that looks like a right one.

The pixels served SHALL be those the application decodes from the file, not the file's own bytes -
so what is drawn is what the enhancement pipeline will later operate on, including a camera RAW's
development and the narrowing of a deeper image to what the transport can carry.

#### Scenario: A bounded rendition

- **WHEN** the pixels of a 3000 x 2000 opened image are requested with a bound of 100
- **THEN** an image 100 pixels on its longest edge is served, with the aspect ratio of the original

#### Scenario: No bound

- **WHEN** the pixels of an opened image are requested with no bound
- **THEN** the image is served at its own dimensions

#### Scenario: A bound larger than the image

- **WHEN** the pixels of a 640 x 480 opened image are requested with a bound of 2000
- **THEN** the image is served at 640 x 480 and is not enlarged

#### Scenario: A crop with no bound

- **WHEN** the pixels of an opened image are requested with a crop and no bound
- **THEN** the framed pixels are served, at the dimensions the crop's rectangle describes

#### Scenario: A crop with a bound

- **WHEN** the pixels of an opened image are requested with a crop and a bound smaller than the
  framing
- **THEN** the framing is served with its longest edge at the bound, showing what the unbounded
  request with the same crop shows

#### Scenario: A crop that cannot be read

- **WHEN** a request carries a crop the application cannot parse
- **THEN** it is refused, rather than the whole photograph being served

#### Scenario: A format the interface cannot decode

- **WHEN** the pixels of an opened camera RAW, TIFF or HEIF image are requested
- **THEN** they are served, in a form the interface can draw

#### Scenario: An image with transparency

- **WHEN** the pixels of an opened image carrying an alpha channel are requested
- **THEN** they are served in a form that keeps the transparency, rather than composited away

### Requirement: An identity and a bound always denote the same pixels

Pixels served for a given identity, bound and crop SHALL be the same on every request, and the
interface SHALL be told it may retain them.

An identity describes bytes, so what it addresses cannot change without the identity changing, and a
bound and a crop are carried in the request rather than held by the application — so what a request
denotes cannot go stale while the request stays the same. That is what lets the interface draw the
same thumbnail repeatedly, and switch back to an image it has already shown, without the application
decoding the file again.

Two requests differing only in their crop SHALL be treated as two different requests, so a crop that
changes never shows the framing the previous one asked for.

#### Scenario: The same request twice

- **WHEN** the pixels for one identity and bound are requested twice
- **THEN** the same image is served both times, and the interface is told the response may be reused

#### Scenario: The same crop twice

- **WHEN** the pixels for one identity, bound and crop are requested twice
- **THEN** the same framing is served both times

#### Scenario: A crop that changes

- **WHEN** the pixels for one identity and bound are requested with two different crops
- **THEN** each request is served its own framing

## ADDED Requirements

### Requirement: How a crop frames a photograph

A crop SHALL carry a horizontal flip, a vertical flip, an angle of rotation and the rectangle to
frame, and the application SHALL apply them in that order: flip horizontally, flip vertically,
rotate, cut. The rectangle is expressed in the coordinate space of the rotated photograph, which is
why the order is fixed rather than chosen per request.

A positive angle SHALL turn the photograph clockwise, matching the direction an on-screen rotation
control reports, and the rotated photograph SHALL occupy the canvas its turned corners need rather
than the original's dimensions. Any area of that canvas no source pixel covers SHALL be transparent,
so a corner a rotation exposes is honest rather than filled with an invented colour, and what is
served SHALL keep that transparency rather than compositing it away.

The rectangle SHALL be brought inside the rotated photograph's bounds before it is cut, so a
rectangle that runs past an edge frames what exists rather than failing the request, and what is
served SHALL always be at least one pixel on each edge. The interface cannot ask for pixels that were
never there.

A framing SHALL keep the bit depth of the photograph it was taken from, so a developed camera RAW is
not flattened to eight bits by being framed.

#### Scenario: The three parts are applied in order

- **WHEN** a request carries a crop with a flip, a rotation and a rectangle
- **THEN** the photograph is flipped, then rotated, then cut, and the pixels served are those the
  rectangle frames in the rotated photograph

#### Scenario: The direction a positive angle turns

- **WHEN** a request carries a crop with a positive angle
- **THEN** the photograph served has turned clockwise

#### Scenario: A rectangle that runs past the edge

- **WHEN** a request carries a crop whose rectangle extends beyond the rotated photograph
- **THEN** the rectangle is brought inside those bounds and the framed pixels are served, rather than
  the request being refused

#### Scenario: A rotated photograph's uncovered corners

- **WHEN** a request carries a crop that rotates the photograph and frames a corner no source pixel
  covers
- **THEN** that area is served as transparent rather than as a colour

#### Scenario: A sixteen-bit photograph

- **WHEN** a request carries a crop of a photograph holding sixteen bits per channel
- **THEN** the framing is served without its depth being flattened
