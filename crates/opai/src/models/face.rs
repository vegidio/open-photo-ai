//! What a detected face is, and the set of them two families speak.

// A file directly under `models/` rather than a member of either family, because that is what this is: detection
// produces these values and face recovery consumes them, and neither owns them. The reference implementation puts
// them in its `detection` package and has `facerecovery` import `detection.Face`, which makes the family that happens
// to produce the value its owner — so face recovery depends on the detection *module* while deliberately not
// depending on the detection *artifact*.
//
// Coordinates are `f32` because the graphs emit `float32`, and widening them would invent precision the detector
// never had. A face **quantizes on acceptance** so that it can be compared and hashed as a value, which `FaceRecovery`
// — and therefore `Operation` — needs, since a selection is part of an operation's identity: a type containing raw
// `f32` can be neither `Eq` nor `Hash`, and `f32`'s own `PartialEq` is unsound as an identity anyway.
//
// The rejected alternative rounds none of these values, on the argument that sub-pixel positions are the whole of what
// distinguishes two detection runs: the reference's FP16 and FP32 detectors return the same faces in the same order,
// differing only in sub-pixel positions. That is sound about *detection*, but a hundredth of a pixel is finer than that
// FP16/FP32 difference and finer than any difference a consumer can act on, and quantizing makes the value agree with
// the run cache key rather than only the key agreeing with itself.
//
// The quantization is at the **face**, not at the point. `Point::new` carries whatever it is given, because a point
// is built three times on the way out of the detector — once in the priors' normalised space, once in the detector's
// square, and once in the source image's pixels — and only the last of those is a coordinate anyone would round.
// Rounding the first to a hundredth would be rounding it to six pixels. `Face::new` is the one place a coordinate is
// known to be in the image's own space, so it is the one place that rounds.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::scale::RangeError;

// Two decimal places, which is the resolution the reference implementation's `FacesCacheKey` folds these values into
// the run cache at: it spells every box at `%.2f`.
/// How many quantization steps one whole pixel — and one whole unit of confidence — is divided into.
const STEPS_PER_UNIT: f32 = 100.0;

/// `value` rounded to one step, a tie away from zero.
fn quantize(value: f32) -> f32 {
    // `round` rather than a formatter: a tie rounds away from zero, exactly as the reference's `%.2f` does, where the
    // `{:.2}` the cache tag is written with rounds a tie to even. The two cannot disagree on eighths, because the tag is
    // written from a value that has no third decimal left to break a tie with.
    (value * STEPS_PER_UNIT).round() / STEPS_PER_UNIT
}

/// `value`'s bit pattern, with the two spellings of zero collapsed into one and every `NaN` payload collapsed into
/// one.
fn bits(value: f32) -> u32 {
    // What makes `Point`'s equality total rather than `f32`'s: `-0.0 == 0.0` while the two have different bits, so
    // hashing the bits directly would put one coordinate in two buckets; and `NaN != NaN`, which would leave a point
    // not equal to itself and `Eq` a false promise. Neither value is one the detector produces — this is what keeps the
    // promise true for a point built by hand, rather than an argument that none is.
    if value == 0.0 {
        return 0;
    }

    if value.is_nan() {
        return f32::NAN.to_bits();
    }

    value.to_bits()
}

/// A point in the pixel coordinate space of the image that was detected on.
///
/// Carried as given rather than rounded: the quantization is [`Face::new`]'s and not this constructor's. Equality and
/// hashing are nonetheless **total**, over a canonical bit pattern per axis, so that a `Face` holding points can be a
/// lookup key whatever it was built from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    // Fields are public because there is nothing to protect: any pair of coordinates is a point, so a constructor
    // could only refuse what the detector never produces.
    /// Distance from the left edge of the image, in pixels.
    pub x: f32,
    /// Distance from the top edge of the image, in pixels.
    pub y: f32,
}

impl Point {
    /// The point at `(x, y)`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// This point rounded to a hundredth of a pixel on each axis, which is what [`Face::new`] accepts.
    fn quantized(self) -> Self {
        Self { x: quantize(self.x), y: quantize(self.y) }
    }

    /// The pair equality and hashing are decided on — see [`bits`].
    fn key(self) -> (u32, u32) {
        (bits(self.x), bits(self.y))
    }
}

/// Two points are equal when both axes are, compared over the canonical bit pattern rather than with `f32`'s `==`:
/// the two spellings of zero are one coordinate, and a point carrying `NaN` is equal to itself.
impl PartialEq for Point {
    fn eq(&self, other: &Self) -> bool {
        // Hand-written rather than derived, and mechanically so: a derived `Eq` would demand `f32: Eq`, which does not
        // hold.
        self.key() == other.key()
    }
}

impl Eq for Point {}

/// Hashed on exactly what [`PartialEq`] compares, which is what `Hash`'s contract requires.
impl std::hash::Hash for Point {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

/// An axis-aligned rectangle, given by its top-left and bottom-right corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    // Public fields for the reason `Point`'s are, and named `min` and `max` as the reference's `RectF` names them.
    /// The top-left corner: the smallest coordinate on each axis.
    pub min: Point,
    /// The bottom-right corner: the largest coordinate on each axis.
    pub max: Point,
}

impl Rect {
    /// The rectangle spanning `min` to `max`.
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// This rectangle with both corners rounded to a hundredth of a pixel, which is what [`Face::new`] accepts.
    fn quantized(self) -> Self {
        Self { min: self.min.quantized(), max: self.max.quantized() }
    }
}

/// How confident the detector is that what it found is a face.
///
/// Bounded to [`Confidence::MIN`]-[`Confidence::MAX`] inclusive on acceptance, so a face that exists is a face whose
/// confidence is meaningful.
///
/// **Quantized to two decimal places on acceptance**, as a coordinate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Confidence {
    // The value is a softmax output and is naturally in range, so the bound writes a guarantee down rather than
    // changing a behaviour — the reference carries a bare `float32`.
    //
    // Stored as an integer count of steps rather than as an `f32` — the same storage `Scale`, `Strength` and `Bias`
    // use, for the same reason they give: a value that keys anything has to compare exactly and hash soundly, and
    // `f32` implements neither trait. A confidence travels inside a `Face`, a face travels inside a `FaceRecovery`, and
    // that operation compares by value — so a confidence carried at full `f32` precision would be the one field
    // keeping a selection from being a lookup key. Two hundredths of confidence is far below anything a consumer acts
    // on: the detector's threshold is applied to the raw score before a `Confidence` is built at all.
    /// The quantized confidence in steps of 1/[`STEPS_PER_UNIT`], always within `0..=100`.
    steps: u8,
}

impl Confidence {
    /// The lowest confidence that can be carried: the detector is certain this is not a face.
    pub const MIN: f32 = 0.0;

    /// The highest confidence that can be carried: the detector is certain this is a face.
    pub const MAX: f32 = 1.0;

    /// The value's name, as [`RangeError`] spells it.
    pub const NAME: &'static str = "confidence";

    /// A confidence, refusing anything outside the permitted range.
    ///
    /// # Errors
    ///
    /// Returns [`RangeError`] naming the permitted range when `value` is below [`Confidence::MIN`], above
    /// [`Confidence::MAX`], or not a number.
    pub fn new(value: f32) -> Result<Self, RangeError> {
        // There is no clamping counterpart, unlike the four parameter types: a confidence is never typed into a
        // control, so there is no already-bounded input whose out-of-range value would be an error a caller cannot act
        // on. Anything out of range here is a decoder that read the wrong tensor, which is a mistake to report.
        //
        // A containment test rather than two comparisons, so that `NaN` — which is neither below the minimum nor
        // above the maximum — is refused instead of carried into a value nothing downstream could compare.
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(RangeError {
                parameter: Self::NAME,
                value: f64::from(value),
                min: f64::from(Self::MIN),
                max: f64::from(Self::MAX),
            });
        }

        Ok(Self::quantize(value))
    }

    /// The confidence as the detector reported it, at the value it was quantized to.
    pub fn get(self) -> f32 {
        f32::from(self.steps) / STEPS_PER_UNIT
    }

    /// Quantizes an already-bounded value.
    fn quantize(bounded: f32) -> Self {
        // Private because the bound is what makes the cast lossless.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "bounded to 0.0..=1.0 above, so the rounded product is within 0..=100"
        )]
        Self { steps: (bounded * STEPS_PER_UNIT).round() as u8 }
    }
}

impl TryFrom<f32> for Confidence {
    type Error = RangeError;

    /// The path a deserialized confidence takes, so a persisted or `invoke`d value is validated exactly as a directly
    /// constructed one is.
    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Confidence> for f32 {
    /// The serialized form: a plain number, so a front end reads the confidence the detector reported rather than
    /// this type's storage.
    fn from(confidence: Confidence) -> Self {
        confidence.get()
    }
}

/// One face found in an image: where it is, the five points that orient it, and how sure the detector is.
///
/// A plain `Copy` value of fourteen coordinates and one confidence. Nothing here owns a native resource. The
/// coordinates are in the pixel coordinate space of the image that was detected on, **rounded to a hundredth of a
/// pixel on acceptance** — this is the one place that rounding happens, and every point a `Face` holds has been
/// through it.
///
/// Two faces accepted as the same value are therefore indistinguishable in every later respect: they compare equal,
/// hash alike, and any operation carrying them reports one run cache tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "FaceFields")]
pub struct Face {
    /// The box enclosing the face.
    bounding_box: Rect,
    /// The five landmarks, in the order [`Face::new`] documents.
    landmarks: [Point; Face::LANDMARKS],
    /// How confident the detector is that this is a face.
    confidence: Confidence,
}

impl Face {
    // A property of the RetinaFace graph rather than a policy, which is why the field is an array of exactly this many
    // rather than a `Vec` plus a check: a four- or six-landmark face is unrepresentable instead of a runtime error, the
    // same move the rest of this vocabulary makes.
    /// How many landmark points the detection graph predicts per face.
    pub const LANDMARKS: usize = 5;

    /// The face at `bounding_box`, oriented by `landmarks`, found with `confidence`.
    ///
    /// `landmarks` are read **positionally** and are, in order:
    ///
    /// 1. the left eye,
    /// 2. the right eye,
    /// 3. the nose,
    /// 4. the left mouth corner,
    /// 5. the right mouth corner.
    ///
    /// Whoever aligns a face reads them in that order, so a reordering here would misalign every face silently rather
    /// than fail.
    ///
    /// Every coordinate is rounded to a hundredth of a pixel here. `confidence` arrives already quantized, because
    /// [`Confidence::new`] is where a confidence is accepted.
    pub fn new(bounding_box: Rect, landmarks: [Point; Self::LANDMARKS], confidence: Confidence) -> Self {
        // Infallible, and deliberately: the confidence is already a `Confidence`, so a face that exists is a face whose
        // confidence is meaningful with no second validation path, and every other field accepts whatever the detector
        // reports. The landmark order is transcribed from the reference's `detection/types.go`.
        Self { bounding_box: bounding_box.quantized(), landmarks: landmarks.map(Point::quantized), confidence }
    }

    /// The box enclosing this face, at the coordinates it was quantized to.
    pub const fn bounding_box(&self) -> Rect {
        self.bounding_box
    }

    /// The five landmarks, in the fixed order [`Face::new`] documents.
    pub const fn landmarks(&self) -> [Point; Self::LANDMARKS] {
        self.landmarks
    }

    /// How confident the detector is that this is a face.
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Whether a face recovery can give this face back more detail than the photograph already holds: its box covers
    /// **at most** the square a recovery model restores a face at, 512 x 512.
    ///
    /// Not a round number picked for looking like one: every face-recovery model is published at a static
    /// `[1, 3, 512, 512]` and aligns a face to a 512-pixel template. A face whose box already covers more than that is
    /// *reduced* into the square, restored and pasted back enlarged, so what the model can give back is bounded by
    /// what the reduction threw away — and on a face that was already sharp that is a softening rather than a
    /// restoration. At or below it the model works at or above the detail the photograph holds.
    ///
    /// Compared as an **area** rather than edge against edge: a 1024 x 256 box and a 512 x 512 one cover the same
    /// pixels and are the same amount of face to restore. A face exactly at the square is restorable: it is neither
    /// reduced nor enlarged, so there is nothing to defend it from.
    ///
    /// The one definition of "too large" every caller shares — the autopilot's face-recovery suggestion, and the
    /// default a front end applies to faces nobody has chosen about — so a suggestion survives exactly when the
    /// recovery it adds would restore at least one face by default.
    pub fn restorable(&self) -> bool {
        // Widened before subtracting, so a box whose corners are whole pixels — the only boxes that can land exactly
        // on the bound — is measured exactly.
        let Rect { min, max } = self.bounding_box;
        let area = (f64::from(max.x) - f64::from(min.x)) * (f64::from(max.y) - f64::from(min.y));

        area <= f64::from(super::face_recovery::restore::TILE).powi(2)
    }
}

// Without it a face read back from disk or from an `invoke` payload would keep whatever precision the payload
// spelled, and two faces that `Face::new` would have made one value would stay distinguishable — the invisible half of
// the trap quantizing on acceptance exists to close. The field names are `Face`'s own, so this reads exactly what the
// `Serialize` derived on `Face` itself writes.
/// The shape a serialized face is read through, so that deserialization accepts one exactly as [`Face::new`] does
/// rather than writing the fields straight in.
///
/// The two refusals a payload can meet belong to the field types: an array of other than five landmarks does not
/// deserialize, and [`Confidence`] refuses a value outside its range.
#[derive(Deserialize)]
struct FaceFields {
    bounding_box: Rect,
    landmarks: [Point; Face::LANDMARKS],
    confidence: Confidence,
}

impl From<FaceFields> for Face {
    fn from(fields: FaceFields) -> Self {
        Self::new(fields.bounding_box, fields.landmarks, fields.confidence)
    }
}

// Backed by `Arc<[Face]>` rather than `Vec<Face>`: a face-recovery operation is cloned freely — held by a front end,
// pushed onto an operation list, handed to a cache lookup — and with a `Vec` every clone would copy every face. With
// an `Arc` "a face handed in is the face handed back" needs no defensive copy.
//
// Reference counting does not weaken the property the whole model vocabulary rests on: it holds plain data with no
// `Drop`, no handle and no native resource, so an operation carrying one is still a description of *what* to run that
// a front end can hold and persist independently of any session.
/// The faces a detection run found, or the faces a recovery run is to restore.
///
/// An ordered, possibly empty set: **an empty set is a legitimate value** — an image with no face, or one where the
/// user deselected every face, is a run that recovers nothing rather than a request the system refuses — and **the
/// order is the order it was given**, because a front end that lets a user pick faces refers to them by position and
/// a reordering would silently re-target that selection.
///
/// A clone is a refcount bump, and the contents are immutable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Vec<Face>", into = "Vec<Face>")]
pub struct Faces(Arc<[Face]>);

// Detection is the one family whose result is not an image, and this is where the library names that result. The
// inference seam states the same fact as the associated type `sealed::DataModel::Output`, which `Analysis` sets to
// `Faces`.
/// What a detection run produces.
pub type DetectionOutput = Faces;

impl Faces {
    /// The faces in `faces`, in the order they are yielded.
    pub fn new(faces: impl IntoIterator<Item = Face>) -> Self {
        Self(faces.into_iter().collect())
    }

    /// No faces — a legitimate value, not an error.
    pub fn empty() -> Self {
        Self(Arc::from([]))
    }

    /// The faces, in order.
    pub fn as_slice(&self) -> &[Face] {
        &self.0
    }

    /// The faces, in order.
    pub fn iter(&self) -> std::slice::Iter<'_, Face> {
        self.0.iter()
    }

    /// How many faces this carries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this carries no faces.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// An order-sensitive signature of the bounding boxes, at two decimal places, semicolon-terminated:
    /// `12.34,56.78,90.12,34.56;`. Empty when there are no faces.
    ///
    /// Uses only digits, `.`, `,`, `;` and `-`, so it carries no underscore and cannot be mistaken for a published
    /// artifact name.
    #[cfg(test)]
    pub(crate) fn cache_signature(&self) -> String {
        // Test-only: the cache tag composes the signature into its own buffer through `write_cache_signature`, so the
        // owned form is read by the tests that pin the spelling and by nothing else.
        let mut signature = String::new();
        self.write_cache_signature(&mut signature);
        signature
    }

    /// Appends the signature to `out`, for a caller already building a string.
    pub(crate) fn write_cache_signature(&self, out: &mut String) {
        // The form the cache tag uses: it is one segment of a longer tag, so composing it into the buffer that tag is
        // being built in costs one allocation where handing back a `String` to be copied in costs two.
        //
        // The reference's `FacesCacheKey` verbatim, including both of the things it leaves out. **Bounding boxes
        // only**, because they uniquely identify the deterministically detected faces within an image — landmarks and
        // confidence move with the box and distinguish nothing a box does not. **Two decimals**, because that is far
        // finer than any difference a re-detection of the same image produces, so a box that moved by a hundredth of a
        // pixel does not discard the cached image.
        //
        // Spelled out rather than digested, which is the right choice because this is **not a filename**: it becomes
        // one part of a run cache key that is hashed into a fixed length anyway — alongside the input image's hash and
        // every operation applied before this one — so digesting here would hash something that is about to be hashed
        // again, would distinguish selections only probabilistically, and would cost the legibility every other
        // family's cache tag has.
        use std::fmt::Write as _;

        for face in self.0.iter() {
            let box_ = face.bounding_box;

            // Writing into a `String` cannot fail, so there is no error here to propagate to a caller.
            let _ = write!(out, "{:.2},{:.2},{:.2},{:.2};", box_.min.x, box_.min.y, box_.max.x, box_.max.y);
        }
    }
}

impl Default for Faces {
    /// No faces, which is the value an image with none produces.
    fn default() -> Self {
        Self::empty()
    }
}

impl FromIterator<Face> for Faces {
    fn from_iter<T: IntoIterator<Item = Face>>(faces: T) -> Self {
        Self::new(faces)
    }
}

impl From<Vec<Face>> for Faces {
    /// Also the deserialization path.
    fn from(faces: Vec<Face>) -> Self {
        // Infallible on purpose: every face was validated by its own `Deserialize`, and an empty list is a value rather
        // than a failure.
        Self(Arc::from(faces))
    }
}

impl From<Faces> for Vec<Face> {
    /// Also the serialization path: a plain list, so a front end persists the faces it selected rather than this
    /// type's storage.
    fn from(faces: Faces) -> Self {
        faces.0.to_vec()
    }
}

impl<'a> IntoIterator for &'a Faces {
    type Item = &'a Face;
    type IntoIter = std::slice::Iter<'a, Face>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::test_support::hash_of;
    use super::*;

    /// A face at the given box, with landmarks and a confidence that are not what any test below is checking.
    pub(crate) fn face_at(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Face {
        Face::new(
            Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
            [
                Point::new(min_x + 10.0, min_y + 10.0),
                Point::new(max_x - 10.0, min_y + 10.0),
                Point::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0),
                Point::new(min_x + 12.0, max_y - 10.0),
                Point::new(max_x - 12.0, max_y - 10.0),
            ],
            Confidence::new(0.9).expect("0.9 is in range"),
        )
    }

    #[test]
    fn a_face_is_restorable_up_to_and_including_the_square_a_recovery_model_restores_at() {
        // Exactly at the square: neither reduced nor enlarged, so restorable.
        assert!(face_at(10.0, 20.0, 522.0, 532.0).restorable());
        // One pixel wider: reduced into the square, so not.
        assert!(!face_at(10.0, 20.0, 523.0, 532.0).restorable());
        // An area rather than an edge: a long thin box covering the same pixels as the square is restorable, and one
        // edge past 512 is not by itself a refusal.
        assert!(face_at(0.0, 0.0, 1024.0, 256.0).restorable());
        assert!(!face_at(0.0, 0.0, 1024.0, 257.0).restorable());
        // A small face, the case a restoration exists for.
        assert!(face_at(10.0, 20.0, 110.0, 140.0).restorable());
    }

    #[test]
    fn a_confidence_inside_the_range_is_taken_as_given() {
        // Every value the detector's threshold can let through, at the resolution a confidence is carried at.
        for value in [0.0, 0.5, 0.75, 0.9, 1.0] {
            let confidence = Confidence::new(value).expect("a confidence in range is accepted");
            assert_eq!(confidence.get(), value, "{value} was not carried at the value supplied");
        }
    }

    #[test]
    fn a_confidence_finer_than_two_decimals_is_quantized_on_acceptance() {
        assert_eq!(Confidence::new(0.999_87).expect("in range").get(), 1.0);
        assert_eq!(Confidence::new(0.876_543).expect("in range").get(), 0.88);
    }

    #[test]
    fn two_confidences_that_quantize_alike_are_one_value() {
        let low = Confidence::new(0.876_1).expect("in range");
        let high = Confidence::new(0.876_9).expect("in range");

        assert_eq!(low, high, "two confidences that quantize alike stayed distinguishable");
        assert_eq!(hash_of(&low), hash_of(&high));
    }

    #[test]
    fn a_confidence_outside_the_range_is_refused() {
        for value in [-0.1_f32, 1.5] {
            let error = Confidence::new(value).expect_err("an out-of-range confidence was accepted");

            assert_eq!(error.parameter, "confidence");
            assert_eq!(error.value, f64::from(value));
            assert_eq!((error.min, error.max), (0.0, 1.0), "the error did not name the permitted range");
            assert!(error.to_string().contains("between 0 and 1"), "the message did not state the range: {error}");
        }
    }

    #[test]
    fn a_confidence_that_is_not_a_number_is_refused() {
        // `NaN` is neither below the minimum nor above the maximum, so the containment test rather than two
        // comparisons is what refuses it — and a face carrying one could not be compared with anything.
        assert!(Confidence::new(f32::NAN).is_err());
        assert!(Confidence::new(f32::INFINITY).is_err());
        assert!(Confidence::new(f32::NEG_INFINITY).is_err());
    }

    #[test]
    fn the_published_constants_are_the_ones_the_constructor_enforces() {
        assert!(Confidence::new(Confidence::MIN).is_ok());
        assert!(Confidence::new(Confidence::MAX).is_ok());
        assert!(Confidence::new(Confidence::MIN - 0.001).is_err());
        assert!(Confidence::new(Confidence::MAX + 0.001).is_err());
    }

    #[test]
    fn a_confidence_round_trips_through_its_serialized_form_as_a_plain_number() {
        let confidence = Confidence::new(0.75).expect("in range");
        let json = serde_json::to_string(&confidence).expect("a confidence serializes");

        assert_eq!(json, "0.75");
        assert_eq!(serde_json::from_str::<Confidence>(&json).expect("a confidence deserializes"), confidence);
    }

    #[test]
    fn a_face_carries_exactly_five_landmarks() {
        // The count is checked by the type rather than at run time: the assertion here is that what comes back is
        // the array that went in, and there is no constructor taking four or six — see the deserialization test
        // below for the one path that could have supplied another number.
        let face = face_at(10.0, 20.0, 110.0, 140.0);

        assert_eq!(face.landmarks().len(), Face::LANDMARKS);
        assert_eq!(Face::LANDMARKS, 5);
    }

    #[test]
    fn a_face_reports_back_the_coordinates_it_was_given_where_they_need_no_rounding() {
        // A coordinate already at a hundredth is carried unchanged, which is what makes the quantization a rounding
        // rather than a rescaling: nothing moves that was already on a step.
        let bounding_box = Rect::new(Point::new(10.25, 20.5), Point::new(110.75, 140.0));
        let landmarks = [
            Point::new(30.5, 50.25),
            Point::new(80.75, 50.5),
            Point::new(55.25, 80.5),
            Point::new(38.25, 110.5),
            Point::new(74.75, 110.25),
        ];
        let face = Face::new(bounding_box, landmarks, Confidence::new(0.88).expect("in range"));

        assert_eq!(face.bounding_box(), bounding_box);
        assert_eq!(face.landmarks(), landmarks);
        assert_eq!(face.confidence().get(), 0.88);
    }

    #[test]
    fn a_coordinate_finer_than_two_decimals_is_quantized_on_acceptance() {
        // The scenario the `models` spec states with this value: a box corner at 12.3456 is reported at 12.35.
        // Landmarks go through the same step, because a box rounded while its landmarks were not would put the points
        // that orient a face at a different resolution from the face.
        let face = Face::new(
            Rect::new(Point::new(12.345_6, 20.375), Point::new(110.937_5, 140.062_5)),
            [Point::new(30.567_8, 50.123_4); Face::LANDMARKS],
            Confidence::new(0.9).expect("in range"),
        );

        assert_eq!(face.bounding_box().min, Point::new(12.35, 20.38));
        assert_eq!(face.bounding_box().max, Point::new(110.94, 140.06));
        assert_eq!(face.landmarks()[0], Point::new(30.57, 50.12));
    }

    #[test]
    fn two_faces_differing_below_the_second_decimal_are_one_value() {
        // What the quantization is for, and the property `FaceRecovery` — and therefore `Operation` — rests on: a
        // re-detection that moved a box by a thousandth of a pixel is the same request, not a different one.
        let detected = face_at(12.34, 56.78, 90.12, 34.56);
        let again = face_at(12.340_1, 56.780_2, 90.120_3, 34.560_4);

        assert_eq!(detected, again, "a sub-hundredth difference read as two faces");
        assert_eq!(hash_of(&detected), hash_of(&again), "two equal faces hashed apart");

        // And as a lookup key, which is the use the trait pair exists for.
        let mut selections = std::collections::HashMap::new();
        selections.insert(Faces::new([detected]), "found");
        assert_eq!(selections.get(&Faces::new([again])), Some(&"found"));
    }

    #[test]
    fn a_point_is_equal_to_itself_whatever_it_carries() {
        // `Eq`'s promise, held for the two values `f32`'s own comparison gets wrong — neither is one the detector
        // produces, which is why this is checked rather than argued. A point built by hand can carry either.
        for point in [Point::new(0.0, 0.0), Point::new(-0.0, -0.0), Point::new(f32::NAN, 1.0)] {
            assert_eq!(point, point, "{point:?} was not equal to itself");
            assert_eq!(hash_of(&point), hash_of(&point));
        }

        assert_eq!(Point::new(0.0, 0.0), Point::new(-0.0, -0.0), "the two spellings of zero were two coordinates");
        assert_eq!(hash_of(&Point::new(0.0, 0.0)), hash_of(&Point::new(-0.0, -0.0)));
    }

    #[test]
    fn a_face_round_trips_through_its_serialized_form() {
        let face = face_at(10.125, 20.375, 110.5, 140.25);
        let json = serde_json::to_string(&face).expect("a face serializes");
        let back: Face = serde_json::from_str(&json).expect("a face deserializes");

        assert_eq!(back, face, "{json} did not round-trip to an equal face");
    }

    #[test]
    fn a_serialized_face_carrying_other_than_five_landmarks_is_refused() {
        // The one path that could have produced a four-landmark face, since no constructor can.
        let json = serde_json::to_string(&face_at(10.0, 20.0, 110.0, 140.0)).expect("a face serializes");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("a face is an object");
        let landmarks = value.get_mut("landmarks").expect("a face carries landmarks").as_array_mut().expect("a list");

        landmarks.pop().expect("five landmarks were serialized");
        let four = serde_json::to_string(&value).expect("the rewritten face serializes");

        assert!(
            serde_json::from_str::<Face>(&four).is_err(),
            "deserialization produced a face with a landmark count no constructor can build: {four}"
        );
    }

    #[test]
    fn a_serialized_face_with_a_confidence_out_of_range_is_refused() {
        let json = serde_json::to_string(&face_at(10.0, 20.0, 110.0, 140.0)).expect("a face serializes");
        let out_of_range = json.replace("\"confidence\":0.9", "\"confidence\":1.5");

        assert_ne!(out_of_range, json, "the test did not rewrite the confidence it meant to");
        assert!(
            serde_json::from_str::<Face>(&out_of_range).is_err(),
            "deserialization produced a face whose confidence could not have been constructed"
        );
    }

    #[test]
    fn an_empty_set_of_faces_is_a_legitimate_value() {
        let faces = Faces::empty();

        assert!(faces.is_empty());
        assert_eq!(faces.len(), 0);
        assert_eq!(faces.as_slice(), &[]);
        assert_eq!(Faces::default(), faces);
        assert_eq!(Faces::new(Vec::new()), faces);
    }

    #[test]
    fn a_set_of_faces_preserves_the_order_it_was_given() {
        let first = face_at(10.0, 10.0, 50.0, 50.0);
        let second = face_at(200.0, 100.0, 260.0, 170.0);
        let third = face_at(5.0, 400.0, 45.0, 450.0);
        let faces = Faces::new([first, second, third]);

        assert_eq!(faces.as_slice(), &[first, second, third]);
        assert_eq!(faces.iter().copied().collect::<Vec<_>>(), vec![first, second, third]);
        assert_ne!(Faces::new([third, second, first]), faces, "a reordered selection compared as the same set");
    }

    #[test]
    fn a_set_of_faces_can_be_built_from_an_iterator_or_a_vec() {
        let built = vec![face_at(10.0, 10.0, 50.0, 50.0), face_at(60.0, 10.0, 100.0, 50.0)];

        assert_eq!(Faces::from(built.clone()), Faces::new(built.clone()));
        assert_eq!(built.iter().copied().collect::<Faces>(), Faces::new(built.clone()));
        assert_eq!(Vec::<Face>::from(Faces::new(built.clone())), built);
    }

    #[test]
    fn a_clone_is_the_same_set_of_faces() {
        // The property the `Arc` buys: cloning a selection is a refcount bump, and the clone is indistinguishable
        // from what it was cloned from.
        let faces = Faces::new([face_at(10.0, 10.0, 50.0, 50.0), face_at(200.0, 100.0, 260.0, 170.0)]);
        let clone = faces.clone();

        assert_eq!(clone, faces);
        assert_eq!(clone.cache_signature(), faces.cache_signature());
    }

    #[test]
    fn the_signature_spells_each_bounding_box_out_at_two_decimals_in_order() {
        // Pinned as a literal, because this is what every cached face-recovery image on every user's disk is keyed
        // through: a change to the spelling invalidates all of them.
        let faces = Faces::new([face_at(12.34, 56.78, 90.12, 34.56), face_at(100.0, 200.5, 300.25, 400.125)]);

        assert_eq!(faces.cache_signature(), "12.34,56.78,90.12,34.56;100.00,200.50,300.25,400.13;");
    }

    #[test]
    fn a_coordinate_exactly_halfway_rounds_away_from_zero_and_does_so_before_the_tag_is_written() {
        // Left to the formatter, `{:.2}` rounds a tie to even, so 400.125 would render `400.12` where the reference's
        // `%.2f` renders `400.13`. The value is rounded on acceptance with `round`, which rounds a tie away from zero,
        // so the two agree and the tag does not depend on its formatter at all.
        // Only the eighths are exact ties: every other "half" a decimal literal suggests is a binary fraction just
        // above or below the midpoint, and rounds the way its actual value says.
        let halfway = Faces::new([face_at(0.125, 0.375, 0.625, 0.875)]);

        assert_eq!(halfway.cache_signature(), "0.13,0.38,0.63,0.88;");

        // The rule is a property of the value: the coordinates themselves report the rounded figures, not only the
        // string written from them.
        let face = Face::new(
            Rect::new(Point::new(0.125, 0.375), Point::new(0.625, 0.875)),
            [Point::new(0.125, 0.375); Face::LANDMARKS],
            Confidence::new(0.9).expect("in range"),
        );
        assert_eq!(face.bounding_box().min, Point::new(0.13, 0.38));
        assert_eq!(face.bounding_box().max, Point::new(0.63, 0.88));
    }

    #[test]
    fn the_signature_of_no_faces_is_empty() {
        // The reference returns `""` here too; what is done with it — an explicit marker rather than nothing at all —
        // is the cache tag's decision, not this one's.
        assert_eq!(Faces::empty().cache_signature(), "");
    }

    #[test]
    fn the_signature_carries_only_characters_an_artifact_name_never_does() {
        let faces = Faces::new([face_at(-12.5, 0.0, 90.125, 34.5), face_at(1.0, 2.0, 3.0, 4.0)]);

        assert!(!faces.cache_signature().contains('_'), "the signature is spelled the way an artifact name is");
        assert!(
            faces
                .cache_signature()
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ';' | '-')),
            "the signature carried a character outside the set it documents: {}",
            faces.cache_signature()
        );
    }

    #[test]
    fn a_set_of_faces_round_trips_through_its_serialized_form_as_a_plain_list() {
        let faces = Faces::new([face_at(10.0, 20.0, 110.0, 140.0), face_at(200.0, 100.0, 260.0, 170.0)]);
        let json = serde_json::to_string(&faces).expect("a set of faces serializes");
        let back: Faces = serde_json::from_str(&json).expect("a set of faces deserializes");

        assert!(json.starts_with('['), "a selection did not serialize as a plain list: {json}");
        assert_eq!(back, faces);
        assert_eq!(back.cache_signature(), faces.cache_signature());

        let empty: Faces = serde_json::from_str("[]").expect("an empty selection deserializes");
        assert!(empty.is_empty());
    }

    #[test]
    fn a_deserialized_face_is_quantized_exactly_as_a_constructed_one_is() {
        let json = r#"{"bounding_box":{"min":{"x":12.3456,"y":20.375},"max":{"x":110.9375,"y":140.0625}},
                       "landmarks":[{"x":30.5678,"y":50.1234},{"x":30.5678,"y":50.1234},{"x":30.5678,"y":50.1234},
                                    {"x":30.5678,"y":50.1234},{"x":30.5678,"y":50.1234}],
                       "confidence":0.876543}"#;
        let face: Face = serde_json::from_str(json).expect("a face deserializes");

        assert_eq!(face.bounding_box().min, Point::new(12.35, 20.38));
        assert_eq!(face.landmarks()[0], Point::new(30.57, 50.12));
        assert_eq!(face.confidence().get(), 0.88);

        // And a second pass does not move it again: rounding an already-rounded value is the identity, which is what
        // makes a persisted selection stable rather than drifting on each round trip.
        let again: Face =
            serde_json::from_str(&serde_json::to_string(&face).expect("a face serializes")).expect("deserializes");
        assert_eq!(again, face);
    }
}
