//! Which families an analysis was asked to check, and so which of its reads it has to make.
//!
//! ```text
//!  read       families it serves
//!  --------   ------------------------------------------------
//!  geometry   Upscale
//!  pass       LightAdjustment, ColorBalance, Colorization
//!  blocks     Denoise, Sharpen
//!  face       FaceRecovery
//! ```

// Owned and `Copy` rather than the caller's slice, because the pixel reads run in a `'static` blocking closure that
// cannot borrow it. A membership set over `Family::ALL` makes duplicates harmless and lets `Detection` match nothing,
// with no special case for either.

use crate::models::Family;

/// The families an analysis checks: every one, or the set its caller named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Scope {
    /// One bit per family, at its position in [`Family::ALL`].
    members: u16,
    /// Whether the caller named a set, which the log reports even when the set happens to hold every family.
    narrowed: bool,
}

impl Scope {
    /// The scope `families` names: `None` is every family, and `Some` exactly the ones listed.
    ///
    /// An empty slice is **not** every family. It is none, so a caller that asked for nothing does not pay for the
    /// most expensive analysis there is.
    pub(super) fn of(families: Option<&[Family]>) -> Self {
        Self {
            members: families.unwrap_or(&Family::ALL).iter().fold(0, |members, family| members | bit(*family)),
            narrowed: families.is_some(),
        }
    }

    /// Whether `family` is checked.
    pub(super) fn includes(self, family: Family) -> bool {
        self.members & bit(family) != 0
    }

    /// Whether the caller named a set, rather than leaving the analysis to check every family.
    pub(super) fn narrowed(self) -> bool {
        self.narrowed
    }

    /// The families checked, in [`Family::ALL`] order.
    pub(super) fn families(self) -> impl Iterator<Item = Family> {
        Family::ALL.into_iter().filter(move |family| self.includes(*family))
    }

    /// Whether the size signal is read.
    pub(super) fn needs_geometry(self) -> bool {
        self.includes(Family::Upscale)
    }

    /// Whether the strided pass is read, for the light, colour and monochrome signals.
    pub(super) fn needs_pass(self) -> bool {
        [Family::LightAdjustment, Family::ColorBalance, Family::Colorization]
            .into_iter()
            .any(|family| self.includes(family))
    }

    /// Whether the block pass is read, for the noise and sharpness signals.
    pub(super) fn needs_blocks(self) -> bool {
        [Family::Denoise, Family::Sharpen].into_iter().any(|family| self.includes(family))
    }

    /// Whether the face signal runs, which installs, opens and runs a detection model.
    pub(super) fn needs_face(self) -> bool {
        self.includes(Family::FaceRecovery)
    }

    /// Whether any read is needed at all. Where none is, no family the scope holds can be suggested.
    pub(super) fn needs_anything(self) -> bool {
        self.needs_geometry() || self.needs_pass() || self.needs_blocks() || self.needs_face()
    }
}

/// `family`'s bit in [`Scope::members`].
fn bit(family: Family) -> u16 {
    // `position` cannot miss, since `Family::ALL` is every family; the fallback is a bit no family reads.
    let position = Family::ALL.iter().position(|candidate| *candidate == family).unwrap_or(15);
    1 << position
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every family some signal can suggest.
    fn suggestible() -> impl Iterator<Item = Family> {
        Family::ALL.into_iter().filter(|family| *family != Family::Detection)
    }

    #[test]
    fn no_set_includes_every_family_and_is_not_narrowed() {
        let scope = Scope::of(None);

        assert!(Family::ALL.into_iter().all(|family| scope.includes(family)));
        assert!(!scope.narrowed());
        assert!(scope.needs_geometry() && scope.needs_pass() && scope.needs_blocks() && scope.needs_face());
    }

    #[test]
    fn an_empty_set_or_one_of_detection_alone_includes_nothing_suggestible_and_needs_no_read() {
        for families in [&[][..], &[Family::Detection][..]] {
            let scope = Scope::of(Some(families));

            assert!(scope.narrowed(), "{families:?}");
            assert!(!suggestible().any(|family| scope.includes(family)), "{families:?}");
            assert!(!scope.needs_anything(), "{families:?}");
        }
    }

    #[test]
    fn duplicates_are_harmless() {
        let once = Scope::of(Some(&[Family::Sharpen, Family::Upscale]));
        let twice = Scope::of(Some(&[Family::Upscale, Family::Sharpen, Family::Sharpen, Family::Upscale]));

        assert_eq!(once, twice);
        assert_eq!(twice.families().collect::<Vec<_>>(), [Family::Sharpen, Family::Upscale]);
    }

    /// Each read is needed exactly when one of the families it serves is included, checked for every family alone.
    #[test]
    fn each_read_is_needed_exactly_when_a_family_it_serves_is_included() {
        for family in Family::ALL {
            let scope = Scope::of(Some(&[family]));

            assert_eq!(scope.needs_geometry(), family == Family::Upscale, "{family:?}");
            assert_eq!(
                scope.needs_pass(),
                matches!(family, Family::LightAdjustment | Family::ColorBalance | Family::Colorization),
                "{family:?}"
            );
            assert_eq!(scope.needs_blocks(), matches!(family, Family::Denoise | Family::Sharpen), "{family:?}");
            assert_eq!(scope.needs_face(), family == Family::FaceRecovery, "{family:?}");
            assert_eq!(scope.needs_anything(), family != Family::Detection, "{family:?}");
        }
    }

    #[test]
    fn a_set_holding_every_family_is_still_narrowed() {
        let scope = Scope::of(Some(&Family::ALL));

        assert!(scope.narrowed());
        assert_eq!(scope.families().collect::<Vec<_>>(), Family::ALL);
    }
}
