## MODIFIED Requirements

### Requirement: Osaka resolves to its three named graphs

An Osaka operation SHALL resolve to three graphs addressed by role - a VAE encoder, a diffusion
transformer, and a VAE decoder - rather than to a sequence of scaling passes. All three SHALL be
required together, since they are stages of one pass.

Only the diffusion transformer SHALL follow the operation's precision. The two VAE halves are
published only in FP16 and SHALL resolve to their FP16 artifacts whatever precision the operation
asks for, because no INT8 build of them exists.

Graphs SHALL be addressed by role rather than by position, so that reordering them cannot silently
exchange the encoder for the decoder.

Each resolved graph SHALL carry the execution-provider settings measured for that graph alongside the
artifact serving it. The two travel together for the same reason the role and the artifact do: a
graph opened under another graph's configuration is not an error anything downstream can detect - it
produces a working session and a slower or wronger result - so the settings must not be something a
caller looks up beside the artifact and could look up wrongly.

#### Scenario: An FP16 Osaka operation requires three FP16 artifacts
- **WHEN** Osaka is requested at FP16
- **THEN** the required artifacts are `up_osaka_fp16`, `up_osaka_vae_encoder_fp16` and
  `up_osaka_vae_decoder_fp16`

#### Scenario: An INT8 Osaka operation quantizes only the transformer
- **WHEN** Osaka is requested at INT8
- **THEN** the required artifacts are `up_osaka_int8`, `up_osaka_vae_encoder_fp16` and
  `up_osaka_vae_decoder_fp16` - the VAE halves staying at FP16

#### Scenario: A role that was never declared is reported rather than ignored
- **WHEN** a graph is requested by a role the variant does not declare
- **THEN** an error naming the role is returned, rather than an absent graph that would fail later

#### Scenario: Each resolved graph carries its own settings
- **WHEN** an Osaka operation is resolved
- **THEN** each of the three graphs reports the settings measured for it together with its artifact
- **AND** the transformer's settings differ from the two VAE halves' where the measurements differ
