//! The fixtures this crate's tests share: the sample input they load, and the faces they report over.
//!
//! A file of its own because these are cross-module. The JSON document, the rendered report and the selection all
//! build the same detected face and load the same embedded picture, and three copies of either is three things that
//! can quietly stop being the same — at which point two tests disagree about what "a face" was and neither of them
//! is wrong.

use opai::{Confidence, Face, Point, Rect};

use crate::input::{self, Input};

/// The embedded sample, decoded.
pub(crate) async fn sample() -> Input {
    input::load(None).await.expect("the embedded sample decodes")
}

/// A face at a plausible box, for the conditions a detection pass reports.
///
/// Its landmarks are the box's own corner repeated, because every caller of this asks about the box, the count or
/// the confidence and none of them about where the eyes are. A test that *is* about the landmarks builds its own —
/// see `select`'s, whose points are spread across the box because the operation it builds carries them.
pub(crate) fn face(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Face {
    Face::new(
        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
        [Point::new(min_x, min_y); 5],
        Confidence::new(0.9).expect("0.9 is in range"),
    )
}
