//! The numeric precision a model artifact is published in.

use serde::{Deserialize, Serialize};

/// The precision one model artifact is built at.
///
/// The common currency the rest of the vocabulary speaks: artifact names are composed from it and display names
/// render it. It says nothing about which variant admits which precision — that is a fact about the files published
/// on HuggingFace, and it lives in the per-contract enums below rather than in a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// Single-precision floating point, the precision every convolutional model is exported at first.
    Fp32,
    /// Half-precision floating point.
    Fp16,
    /// A weight-only quantized build: the weights are stored as `int8` and the activations are not, so it is a
    /// smaller download and a faster run rather than a different graph.
    Int8,
}

impl Precision {
    /// How the precision is spelled inside an artifact name: `fp32`.
    ///
    /// Lower case, because it ends up in a file name and a download URL. This is the only place the spelling is
    /// written, so the names this composes and the names published on HuggingFace cannot drift into two spellings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fp32 => "fp32",
            Self::Fp16 => "fp16",
            Self::Int8 => "int8",
        }
    }

    /// How the precision is spelled in a display name: `FP32`.
    ///
    /// A second constant rather than an upper-casing of the first, because the two are read by different audiences
    /// and only one of them is composed into a path.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fp32 => "FP32",
            Self::Fp16 => "FP16",
            Self::Int8 => "INT8",
        }
    }
}

impl std::fmt::Display for Precision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The precisions a model published as floating point, at either width, is available in.
///
/// Nineteen of the twenty variants the library names use this contract: the three convolutional upscalers, the
/// thirteen enhancement variants, and the detection and face recovery variants all ship FP32 and FP16 weights and
/// nothing else. Pairing one of them with INT8 is not a runtime error here — it does not compile, so
/// `dn_stockholm_int8` has no path that could name it.
///
/// Named for the domain rather than for the three variants that happened to reach it first: "convolutional" says
/// nothing on a colorization or colour-balance model, and a second identical enum per family would be one domain fact
/// written eight times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FloatPrecision {
    /// Single-precision floating point.
    Fp32,
    /// Half-precision floating point.
    Fp16,
}

impl FloatPrecision {
    /// Every precision this contract admits, in the order a chooser should offer them.
    ///
    /// The one source the catalogue reads, so a control cannot offer a precision resolution would refuse.
    pub const ALL: [Self; 2] = [Self::Fp32, Self::Fp16];

    /// The same list, widened to the currency artifact names and the catalogue are composed from.
    ///
    /// It sits here, with the domain it widens, rather than beside the catalogue that reads it: which variant admits
    /// which precisions is a fact about the files published on HuggingFace, and deciding it outside the model is the
    /// drift the catalogue exists to prevent. Written out rather than mapped from [`ALL`](Self::ALL), which `const`
    /// evaluation cannot do; the test below is what keeps the two from disagreeing.
    pub(crate) const PRECISIONS: [Precision; 2] = [Precision::Fp32, Precision::Fp16];

    /// The precision of this contract `precision` names, or `None` where it names one outside it.
    ///
    /// Fallible, and that is the whole of it: a precision arriving from the catalogue is the widened kind, and the
    /// answer for INT8 is that these models publish no such build rather than that one can be narrowed to. Nothing
    /// here lets an INT8 request reach an artifact name — it stops at the `None`.
    pub(crate) const fn narrow(precision: Precision) -> Option<Self> {
        match precision {
            Precision::Fp32 => Some(Self::Fp32),
            Precision::Fp16 => Some(Self::Fp16),
            Precision::Int8 => None,
        }
    }
}

impl From<FloatPrecision> for Precision {
    /// Widening, and infallible in this direction: every floating-point precision is a precision. The reverse does not
    /// exist, which is what keeps INT8 out of the artifact names of the variants published only as floating point.
    fn from(precision: FloatPrecision) -> Self {
        match precision {
            FloatPrecision::Fp32 => Self::Fp32,
            FloatPrecision::Fp16 => Self::Fp16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::upscale::osaka::precision::OsakaPrecision;

    #[test]
    fn each_precision_renders_the_string_its_filenames_use() {
        // These three strings are the tail of every artifact name the project publishes; a fourth spelling of any of
        // them names a file that does not exist.
        assert_eq!(Precision::Fp32.as_str(), "fp32");
        assert_eq!(Precision::Fp16.as_str(), "fp16");
        assert_eq!(Precision::Int8.as_str(), "int8");
    }

    #[test]
    fn display_renders_the_same_string_as_the_filename_spelling() {
        assert_eq!(Precision::Fp16.to_string(), Precision::Fp16.as_str());
    }

    #[test]
    fn each_precision_renders_an_upper_case_display_label() {
        assert_eq!(Precision::Fp32.label(), "FP32");
        assert_eq!(Precision::Fp16.label(), "FP16");
        assert_eq!(Precision::Int8.label(), "INT8");
    }

    #[test]
    fn the_floating_point_contract_widens_to_the_matching_precision() {
        assert_eq!(Precision::from(FloatPrecision::Fp32), Precision::Fp32);
        assert_eq!(Precision::from(FloatPrecision::Fp16), Precision::Fp16);
    }

    #[test]
    fn each_contract_narrows_exactly_the_precisions_it_publishes_and_refuses_the_rest() {
        // The direction the catalogue's inverse reads in: a row publishes a widened precision, and narrowing it is
        // what decides whether the pairing names a variant at all.
        for precision in FloatPrecision::ALL {
            assert_eq!(FloatPrecision::narrow(precision.into()), Some(precision));
        }
        for precision in OsakaPrecision::ALL {
            assert_eq!(OsakaPrecision::narrow(precision.into()), Some(precision));
        }

        assert_eq!(FloatPrecision::narrow(Precision::Int8), None, "a float-only model narrowed to an INT8 build");
        assert_eq!(OsakaPrecision::narrow(Precision::Fp32), None, "Osaka narrowed to the FP32 build it has none of");
    }

    #[test]
    fn each_contract_publishes_the_precisions_it_admits() {
        assert_eq!(FloatPrecision::ALL, [FloatPrecision::Fp32, FloatPrecision::Fp16]);
        assert_eq!(OsakaPrecision::ALL, [OsakaPrecision::Fp16, OsakaPrecision::Int8]);
    }

    #[test]
    fn each_contracts_widened_list_is_the_list_it_admits_and_in_the_same_order() {
        // The two are written out separately because `const` evaluation cannot map one into the other, so this is
        // what stops a precision added to one from being missing from what a chooser reads.
        assert_eq!(FloatPrecision::PRECISIONS, FloatPrecision::ALL.map(Precision::from));
        assert_eq!(OsakaPrecision::PRECISIONS, OsakaPrecision::ALL.map(Precision::from));
    }

    #[test]
    fn a_precision_round_trips_through_its_serialized_form() {
        // The serialized spelling is the lower-case one, because a persisted setting and an artifact name should not
        // disagree about how a precision is written.
        let json = serde_json::to_string(&Precision::Int8).expect("a precision serializes");
        assert_eq!(json, "\"int8\"");
        assert_eq!(serde_json::from_str::<Precision>(&json).expect("a precision deserializes"), Precision::Int8);
    }
}
