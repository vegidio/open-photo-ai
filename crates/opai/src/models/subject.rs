//! The union of the two carriers: an operation of either run path, as everything that reports on a run names it.

use super::artifact::{ArtifactId, Family};
use super::operation::{Analysis, Operation};
use super::precision::Precision;

// A progress report has to be able to name either carrier, because both paths report. So a report's subject is this
// rather than one of the two, and detection reports exactly as every other family does: its constructor returns its
// carrier and its report carries that carrier.
//
// A reporting vocabulary that nothing dispatches on, which is why neither path's entry point takes one. Here beside the
// carriers rather than beside the progress report that carries it, because the public catalogue's `build` returns one
// too — it is the carriers' own union, and a `models` item that `models` had to import from the orchestrator would
// point the dependency the wrong way.
/// What one run is about: an operation of either of the library's two run paths.
///
/// The same shape [`Operation`] itself is: a union that forwards the five questions every family answers, so a
/// consumer that only labels a bar calls [`display_name`](Self::display_name) and never matches at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    /// An operation whose result is an image, as [`Opai::process`](crate::Opai::process) runs.
    Enhancement(Operation),
    /// An operation whose result is not an image, as [`Opai::execute`](crate::Opai::execute) runs.
    Analysis(Analysis),
}

// Written once for the same reason `Operation`'s own forwarding is: five copies of a two-arm match are five chances
// to wire an arm to the other carrier's value, and a mistake that compiles.
/// Forwards a question both carriers answer to the one being carried.
macro_rules! forward {
    ($subject:ident, $question:ident) => {
        match $subject {
            Self::Enhancement(operation) => operation.$question(),
            Self::Analysis(analysis) => analysis.$question(),
        }
    };
}

impl Subject {
    /// Which family this run is of.
    pub fn family(&self) -> Family {
        forward!(self, family)
    }

    /// The precision this run is at, widened to the common currency artifact names are composed from.
    pub fn precision(&self) -> Precision {
        forward!(self, precision)
    }

    /// The name a user interface shows: `Kyoto 4x (FP16)`, `New York (FP32)`.
    pub fn display_name(&self) -> String {
        forward!(self, display_name)
    }

    /// The artifacts that must be on disk before this run can proceed.
    pub fn required_artifacts(&self) -> Vec<ArtifactId> {
        forward!(self, required_artifacts)
    }

    /// The key the store keeps this run's result under.
    pub fn cache_tag(&self) -> String {
        forward!(self, cache_tag)
    }
}

impl From<Operation> for Subject {
    fn from(operation: Operation) -> Self {
        Self::Enhancement(operation)
    }
}

impl From<Analysis> for Subject {
    fn from(analysis: Analysis) -> Self {
        Self::Analysis(analysis)
    }
}
