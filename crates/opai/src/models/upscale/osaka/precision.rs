//! The precisions this model is published in.
//!
//! Its own contract rather than a shared enum plus a per-variant check, and its own file beside the model that
//! publishes it: [`FloatPrecision`](crate::models::precision::FloatPrecision) serves the other nineteen variants the
//! library names and says nothing about this one. What the two share is [`Precision`], the widened currency artifact
//! names are composed from, which stays cross-family.

use serde::{Deserialize, Serialize};

use crate::models::precision::Precision;

/// The precisions Osaka is published in.
///
/// FP16 and INT8. There is no FP32 build of the diffusion transformer, so `Osaka` paired with FP32 is unrepresentable
/// rather than rejected — the artifact `up_osaka_fp32` has no construction path to name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsakaPrecision {
    /// Half-precision floating point: the full-weight build.
    Fp16,
    /// The quantized transformer — half the download and about twice as fast on the CPU. Only the transformer is
    /// quantized; its two VAE halves stay FP16, which is why [`crate::Upscale`] resolution pins them.
    Int8,
}

impl OsakaPrecision {
    /// Every precision this contract admits, in the order a chooser should offer them.
    pub const ALL: [Self; 2] = [Self::Fp16, Self::Int8];

    /// The same list, widened to the currency artifact names and the catalogue are composed from.
    ///
    /// Its own list beside its own contract, mirroring
    /// [`FloatPrecision::PRECISIONS`](crate::models::precision::FloatPrecision::PRECISIONS): a catalogue that decided
    /// which of the two a model gets would be silently wrong the moment a second diffusion model published a
    /// different pair.
    pub(crate) const PRECISIONS: [Precision; 2] = [Precision::Fp16, Precision::Int8];

    /// The precision of this contract `precision` names, or `None` where it names one outside it.
    ///
    /// FP32 is the `None` here, mirroring INT8 in [`FloatPrecision::narrow`](crate::models::FloatPrecision::narrow): the two contracts refuse the precision
    /// the other admits, which is what keeps a catalogue row from being read back as a build that was never
    /// published.
    pub(crate) const fn narrow(precision: Precision) -> Option<Self> {
        match precision {
            Precision::Fp16 => Some(Self::Fp16),
            Precision::Int8 => Some(Self::Int8),
            Precision::Fp32 => None,
        }
    }
}

impl From<OsakaPrecision> for Precision {
    fn from(precision: OsakaPrecision) -> Self {
        match precision {
            OsakaPrecision::Fp16 => Self::Fp16,
            OsakaPrecision::Int8 => Self::Int8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_osaka_contract_widens_to_the_matching_precision() {
        assert_eq!(Precision::from(OsakaPrecision::Fp16), Precision::Fp16);
        assert_eq!(Precision::from(OsakaPrecision::Int8), Precision::Int8);
    }

    #[test]
    fn no_construction_path_produces_an_osaka_fp32() {
        // The guarantee is a compile-time one — `OsakaPrecision` has no FP32 variant, so there is nothing to call
        // this with that would produce one. What is checked here is that widening every precision Osaka *can* be
        // built with stays away from FP32, which is what would name `up_osaka_fp32`.
        for precision in OsakaPrecision::ALL {
            assert_ne!(
                Precision::from(precision),
                Precision::Fp32,
                "Osaka widened to a precision it is not published in"
            );
        }
    }

    #[test]
    fn a_serialized_osaka_precision_of_fp32_is_refused() {
        assert!(
            serde_json::from_str::<OsakaPrecision>("\"fp32\"").is_err(),
            "Osaka accepted a precision it is not published in"
        );
    }
}
