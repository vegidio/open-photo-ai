## ADDED Requirements

### Requirement: A run enhances the image as the user has framed it

A request to enhance an open image SHALL be able to carry that image's crop, and the chain SHALL run
over the framed pixels rather than over the whole photograph.

The result of such a run SHALL be identified by what produced it — the framed pixels and the chain —
so that the same chain run over two different framings of one photograph answers two different
results, and neither can be served where the other was asked for.

Work already done for one framing SHALL NOT be reused for another. A run over a newly framed
photograph SHALL produce the enhancement of that framing, whatever the application has already
computed for the same chain over the same photograph framed differently.

A request carrying no crop SHALL run exactly as it does today, over the whole photograph, and so SHALL
a request carrying a crop that frames the whole photograph unturned and unflipped: a framing that
changes no pixel SHALL cost no work, so opening the framing controls and changing nothing does not
discard everything already computed for that photograph.

#### Scenario: A chain over a framed image

- **WHEN** the interface asks for an admitted image to be enhanced and carries a crop with the request
- **THEN** the chain runs over the framed pixels, and the result's dimensions are those of the
  enhanced framing

#### Scenario: Two framings of one photograph

- **WHEN** one chain is run over an image with one crop and then over the same image with another
- **THEN** the two results carry different identities and each answers its own framing

#### Scenario: A repeated chain over one framing

- **WHEN** a chain that has already been run over a framed image is asked for again with the same crop
- **THEN** the same result is answered

#### Scenario: No crop

- **WHEN** the interface asks for an admitted image to be enhanced and carries no crop
- **THEN** the chain runs over the whole photograph

#### Scenario: A framing that changes nothing

- **WHEN** a chain that has already been run over a photograph is asked for again carrying a crop with
  no flip, no rotation and a rectangle covering the whole photograph
- **THEN** the result already computed is answered, rather than the chain being run a second time

### Requirement: Changing the framing starts a new run and lets go of the previous result

While an image is open, changing its crop SHALL start a new run of that image's chain over the new
framing, and SHALL stop whatever run was in flight.

The result the previous framing produced SHALL be released rather than left on screen while the new
run works: the window draws a photograph at the shape of its own dimensions, and a result made from
other pixels drawn into the new framing's shape would be drawn to the wrong proportions. Until the new
run lands, the window SHALL draw the newly framed source.

This is a deliberate divergence from the reference implementation, which keeps the previous result on
screen. It differs from changing an enhancement, where the previous result is held until a newer one
lands, because an enhancement does not change the shape of what is drawn and a crop does.

#### Scenario: The crop changes while a result is on screen

- **WHEN** the crop of an image carrying an enhanced result is changed
- **THEN** the previous result is released, the window draws the newly framed source, and a run over
  the new framing starts

#### Scenario: The crop changes while a run is in flight

- **WHEN** the crop of an image is changed while a run over the previous framing is running
- **THEN** that run is stopped and a run over the new framing is started

#### Scenario: An image with no enhancements

- **WHEN** the crop of an image carrying no enhancements is changed
- **THEN** no run is asked for, and the window draws the newly framed source
