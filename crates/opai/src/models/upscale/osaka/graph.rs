//! How one pass of this model is addressed: the roles its stages fill, the row declaring them, and what an operation
//! resolves each of them to.

// Role-addressed throughout rather than positional, and that is the whole of why this is a vocabulary rather than a
// list. Reordering a list silently exchanges the encoder and the decoder, which compiles, runs, and returns a wrong
// image with nothing downstream able to catch it.
//
// `resolve` is what the family's `Upscale::resolve` reaches for this contract, through this row's
// `UpscaleModel::resolve`. It lives here, with the types it builds, so that composing this model's artifact names out
// of this model's row under this model's pinning rules is this model's code — a family-tier file that built a
// `GraphSet` would be the coupling this layout exists to remove, spelled as a constructor instead of as a match.

use serde::{Deserialize, Serialize};

use super::precision::OsakaPrecision;
use crate::models::artifact::{ArtifactId, Family};
use crate::models::catalogue::{ParameterEntry, SCALE_PARAMETER};
use crate::models::precision::Precision;
use crate::models::scale::Scale;
use crate::models::upscale::conv::PassShape;
use crate::models::upscale::resolve::Resolution;
use crate::models::upscale::variant::{UpscaleModel, UpscaleVariant};
use crate::providers::profile::EpProfile;

/// How the pipeline asks a diffusion variant for one of its graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphRole {
    /// The VAE encoder, which turns the image into the latent the transformer works on.
    Encoder,
    /// The diffusion transformer, which holds nearly all of the variant's weight and does the restoration.
    Transformer,
    /// The VAE decoder, which turns the restored latent back into an image.
    Decoder,
}

impl GraphRole {
    /// The role's name, as an error reporting an undeclared one shows it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Encoder => "encoder",
            Self::Transformer => "transformer",
            Self::Decoder => "decoder",
        }
    }
}

impl std::fmt::Display for GraphRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One graph of a multi-stage variant: which role it fills, which artifact serves it, and whether its precision is
/// its own rather than the operation's.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Graph {
    /// How the pipeline asks for this graph.
    pub(super) role: GraphRole,
    /// What distinguishes this graph's artifact from the variant's base model name. Empty for the graph that *is*
    /// the base model.
    pub(super) suffix: &'static str,
    // A multi-stage model is not published as one precision throughout: Osaka's transformer is published both as FP16
    // and quantized to INT8, while its two VAE halves are small by comparison and published only as FP16 — one pair of
    // files shared by both builds. Without the pin, an INT8 operation would resolve them to `_int8` names that were
    // never published.
    /// The precision this graph is pinned to regardless of what the operation asked for, or `None` where it follows
    /// the operation.
    pub(super) pinned: Option<Precision>,
    // The same shape as `pinned` and for the same reason: a multi-stage model is not one graph, so a figure measured on
    // the variant is not automatically a figure about each of its stages. Osaka's transformer names the deviation here
    // rather than leaving the pipeline to remember it — see `OSAKA`'s graph list and `osaka::profile` for the figures
    // on both sides.
    //
    // `None` rather than a `bool` defaulting to the variant's value, because the two are different statements: a graph
    // that names nothing is one nobody measured separately.
    /// The CUDA convolution layout this graph's own measurement asks for, or `None` where it carries the variant's
    /// declaration, whatever that declaration later becomes.
    pub(super) cuda_prefer_nhwc: Option<bool>,
}

impl Graph {
    /// How the pipeline asks for this graph.
    pub(crate) const fn role(self) -> GraphRole {
        self.role
    }

    /// The artifact serving this graph when the operation asked for `requested`.
    pub(crate) fn artifact(self, codename: &str, requested: Precision) -> ArtifactId {
        // A method rather than three lines at the one call site so that the pin can be tested without opening a
        // session, which for this family means a multi-gigabyte download.
        let precision = self.pinned.unwrap_or(requested);

        ArtifactId::suffixed(Family::Upscale, codename, self.suffix, precision)
    }

    /// The settings this graph is configured with, given the `variant`'s own declaration.
    pub(crate) fn profile(self, variant: EpProfile) -> EpProfile {
        // Beside `artifact` so that the two things a caller needs about a graph are answered from the one row that
        // declares them — see `ResolvedGraph` for why the settings travel with the graph.
        match self.cuda_prefer_nhwc {
            Some(prefer_nhwc) => EpProfile { cuda_prefer_nhwc: prefer_nhwc, ..variant },
            None => variant,
        }
    }
}

// The error is what makes addressing a graph by name safer than addressing it by position rather than merely
// different: a role the variant never declared is a mistake in the pipeline, and handing back an absent graph for it
// only moves the failure to a null dereference inside the session that was never opened.
/// A graph was asked for by a role the variant does not declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no graph is bound to the role {role} on {variant}")]
pub struct UnknownRole {
    /// The role that was asked for.
    pub role: GraphRole,
    /// The codename of the variant that does not declare it.
    pub variant: &'static str,
}

/// One diffusion upscale variant, as data.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DiffusionVariant {
    // A separate shape from `ConvVariant` rather than a set of optional fields on it, so that the two contracts stay
    // legible side by side: this one holds a fixed set of graphs run as one pass and carries no scale table at all.
    /// The developer-facing identifier; see [`ConvVariant::codename`](crate::models::upscale::conv::ConvVariant#structfield.codename).
    pub(crate) codename: &'static str,
    /// The display text a user sees; see [`ConvVariant::label`](crate::models::upscale::conv::ConvVariant#structfield.label).
    pub(crate) label: &'static str,
    /// The stages loaded together for one pass, addressed by role.
    pub(crate) graphs: &'static [Graph],
    // A function of the precision for the reason `ConvVariant::profile` gives, and with no wrapper either.
    /// The execution-provider tuning measured for this model, at the precision an operation carries. See the model's
    /// own directory for the measurements and the prose recording how they were arrived at.
    pub(crate) profile: fn(Precision) -> EpProfile,
}

// The three travel together rather than being three lookups a caller joins. The role and the artifact travel together
// because reordering a list silently exchanges the encoder and the decoder; the settings travel with them for the same
// reason one step further out — a graph opened under another graph's configuration is not an error anything
// downstream can detect. It produces a working session and a slower or wronger result, so making the settings
// something a caller looks up beside the artifact is making them something a caller can look up wrongly.
/// One resolved graph: the role it fills, the artifact serving it, and the settings it is opened under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGraph {
    /// How the pipeline asks for this graph.
    pub role: GraphRole,
    /// The artifact serving it, at the precision this graph resolves at — which is not always the operation's.
    pub artifact: ArtifactId,
    // Not public, because `EpProfile` is this crate's own vocabulary and no consumer configures a session.
    /// The execution-provider settings measured for **this graph**, which are the variant's unless the graph named a
    /// deviation from them.
    pub(crate) profile: EpProfile,
}

/// The graphs a diffusion variant loads for one pass, addressed by role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSet {
    /// The codename of the variant these came from, so a lookup that fails can name it.
    variant: &'static str,
    /// The declared graphs, in the order the variant lists them.
    graphs: Vec<ResolvedGraph>,
}

impl GraphSet {
    /// The artifact filling `role`.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownRole`] naming the role when this set declares no graph for it.
    pub fn graph(&self, role: GraphRole) -> Result<&ArtifactId, UnknownRole> {
        self.resolved(role).map(|graph| &graph.artifact)
    }

    /// The whole of the graph filling `role` — its artifact and the settings it is opened under.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownRole`] naming the role when this set declares no graph for it.
    pub(crate) fn resolved(&self, role: GraphRole) -> Result<&ResolvedGraph, UnknownRole> {
        self.graphs
            .iter()
            .find(|graph| graph.role == role)
            .ok_or(UnknownRole { role, variant: self.variant })
    }

    /// Every graph this set declares, in the order the variant lists them. All of them are loaded together, since
    /// they are stages of one pass.
    pub(crate) fn resolved_graphs(&self) -> impl Iterator<Item = &ResolvedGraph> {
        self.graphs.iter()
    }

    /// Every role this set declares, in the order the variant lists them.
    #[cfg(test)]
    pub(crate) fn roles(&self) -> impl Iterator<Item = GraphRole> + '_ {
        self.graphs.iter().map(|graph| graph.role)
    }

    /// Every artifact this set needs, in the order the variant lists them.
    pub fn artifacts(&self) -> impl Iterator<Item = &ArtifactId> {
        self.graphs.iter().map(|graph| &graph.artifact)
    }
}

/// The graphs `row` loads for one pass at `precision`.
pub(crate) fn resolve(row: &DiffusionVariant, precision: Precision) -> GraphSet {
    // The variant's declaration is read once and handed to each graph, which is what makes "a graph that
    // names no deviation carries the variant's own settings" a property of this loop rather than something
    // each row restates. See `Graph::profile`.
    let declared = (row.profile)(precision);
    let graphs = row
        .graphs
        .iter()
        .map(|graph: &Graph| ResolvedGraph {
            role: graph.role(),
            artifact: graph.artifact(row.codename, precision),
            profile: graph.profile(declared.clone()),
        })
        .collect();

    GraphSet { variant: row.codename, graphs }
}

impl UpscaleModel for DiffusionVariant {
    fn codename(&self) -> &'static str {
        self.codename
    }

    fn label(&self) -> &'static str {
        self.label
    }

    fn precisions(&self) -> &'static [Precision] {
        &OsakaPrecision::PRECISIONS
    }

    fn parameters(&self) -> &'static [ParameterEntry] {
        &SCALE_PARAMETER
    }

    fn profile(&self, precision: Precision) -> EpProfile {
        (self.profile)(precision)
    }

    /// `None`: this contract runs graphs, at the geometry and range `latent.rs` declares for them.
    fn pass_shape(&self) -> Option<PassShape> {
        None
    }

    fn variant(&self, precision: Precision) -> Option<UpscaleVariant> {
        OsakaPrecision::narrow(precision).map(UpscaleVariant::Osaka)
    }

    /// The graphs, whatever the scale — which is the contract. The scale travels per run instead.
    fn resolve(&self, precision: Precision, _scale: Scale) -> Resolution {
        Resolution::Graphs(resolve(self, precision))
    }
}

#[cfg(test)]
mod tests {
    use super::super::OSAKA;
    use super::*;

    #[test]
    fn osaka_declares_the_three_roles_one_pass_needs() {
        let roles: Vec<GraphRole> = OSAKA.graphs.iter().map(|graph| graph.role()).collect();

        assert_eq!(roles, vec![GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder]);
        assert_eq!(OSAKA.codename, "osaka");
        assert_eq!(OSAKA.label, "Osaka");
    }

    #[test]
    fn the_transformer_follows_the_operations_precision() {
        let transformer = graph(GraphRole::Transformer);

        assert_eq!(transformer.artifact(OSAKA.codename, Precision::Fp16).as_str(), "up_osaka_fp16");
        assert_eq!(transformer.artifact(OSAKA.codename, Precision::Int8).as_str(), "up_osaka_int8");
    }

    #[test]
    fn the_vae_halves_stay_at_fp16_under_an_int8_operation() {
        // No INT8 build of either half exists, so a half that followed the operation would resolve to
        // `up_osaka_vae_encoder_int8` — a file that was never published.
        for (role, expected) in [
            (GraphRole::Encoder, "up_osaka_vae_encoder_fp16"),
            (GraphRole::Decoder, "up_osaka_vae_decoder_fp16"),
        ] {
            for precision in [Precision::Fp16, Precision::Int8] {
                assert_eq!(graph(role).artifact(OSAKA.codename, precision).as_str(), expected, "{role} at {precision}");
            }
        }
    }

    #[test]
    fn every_role_renders_the_name_an_error_would_report() {
        assert_eq!(GraphRole::Encoder.to_string(), "encoder");
        assert_eq!(GraphRole::Transformer.to_string(), "transformer");
        assert_eq!(GraphRole::Decoder.to_string(), "decoder");
    }

    /// Osaka's graph filling `role`.
    fn graph(role: GraphRole) -> Graph {
        *OSAKA.graphs.iter().find(|graph| graph.role() == role).expect("Osaka declares this role")
    }

    #[test]
    fn a_role_the_variant_never_declared_is_reported_rather_than_ignored() {
        // Built here rather than resolved from a published variant because every variant that exists today declares
        // all three roles. What is under test is the guard, which is what a variant published with two stages, or a
        // pipeline asking for a role it invented, would meet — an absent graph handed back instead would only move
        // the failure into a session that was never opened.
        let stage = |role, name| ResolvedGraph {
            role,
            artifact: ArtifactId::new(Family::Upscale, name, None, Precision::Fp16),
            profile: EpProfile::default(),
        };
        let two_staged = GraphSet {
            variant: "nowhere",
            graphs: vec![
                stage(GraphRole::Encoder, "nowhere_vae_encoder"),
                stage(GraphRole::Decoder, "nowhere_vae_decoder"),
            ],
        };

        let error = two_staged.graph(GraphRole::Transformer).expect_err("an undeclared role resolved to a graph");

        assert_eq!(error.role, GraphRole::Transformer);
        assert_eq!(error.variant, "nowhere");
        assert!(error.to_string().contains("transformer"), "the error did not name the role: {error}");
    }
}
