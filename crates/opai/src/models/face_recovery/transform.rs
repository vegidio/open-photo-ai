//! The canonical landmark template, the similarity transform fitted onto it, and its inverse.
//!
//! The geometry a restoration run is built on, and the one piece of it whose mistakes are invisible: a transform
//! fitted to the wrong landmark order, or an inverse applied in the forward direction, produces a plausible picture
//! with a face in the wrong place rather than an error. So every part of it is arithmetic over six floats, tested on
//! its own, before [`warp`](super::warp) samples a pixel through one.
//!
//! Nothing here names a variant or a tile size: the template is expressed at its own reference size and scaled to
//! whatever square the caller's model accepts, which is what both of the family's models need from this file and
//! what made its promotion to family tier a file move.

use crate::models::face::{Face, Point};

/// The edge length the template's coordinates below are expressed in.
///
/// The template is scaled from here to whatever square a model accepts, which is what makes that scaling explicit
/// rather than an assumption that every variant happens to be 512. Both shipping variants run at 512, so the scale
/// is exactly 1 today and the output is unchanged — but leaving the two numbers independent means a variant at
/// another tile size aligns against a template in its own coordinate space rather than against one in another's.
pub(crate) const TEMPLATE_SIZE: f32 = 512.0;

/// The canonical ArcFace landmark layout, at [`TEMPLATE_SIZE`].
///
/// Literals transcribed from the reference's `detection/types.go`, in the order [`Face::new`] documents its
/// landmarks: left eye, right eye, nose, left mouth corner, right mouth corner. **The order is the whole of the
/// contract.** A face's landmarks are read positionally and fitted against this positionally, so exchanging two
/// entries here misaligns every face in every photograph and reports nothing.
pub(crate) const TEMPLATE: [Point; Face::LANDMARKS] = [
    Point::new(192.98, 239.95),
    Point::new(318.90, 240.19),
    Point::new(256.63, 314.02),
    Point::new(201.26, 371.41),
    Point::new(313.09, 371.15),
];

/// Below this, a source point set carries no scale and no rotation to fit — see [`similarity`].
const DEGENERATE_NORM: f32 = 1e-10;

/// Below this, a matrix's determinant is treated as zero rather than inverted — see [`Affine::inverse`].
const SINGULAR_DET: f32 = 1e-10;

/// A 2x3 affine transform, as the reference's `AffineMatrix` is: two rows of three coefficients, applied to a point
/// as `(a00 x + a01 y + a02, a10 x + a11 y + a12)`.
///
/// `f32` throughout, and fitted and applied in `f32`, because the reference's is. `f64` would be defensible on its
/// own merits and would make every pixel-level comparison against the reference approximate instead of exact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Affine {
    /// The two rows, each `[scale-or-rotation, scale-or-rotation, translation]`.
    pub(crate) rows: [[f32; 3]; 2],
}

impl Affine {
    /// The transform that leaves every point where it is.
    pub(crate) const IDENTITY: Self = Self { rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] };

    /// The pure translation by `(x, y)`.
    const fn translation(x: f32, y: f32) -> Self {
        Self { rows: [[1.0, 0.0, x], [0.0, 1.0, y]] }
    }

    /// Where this transform sends `(x, y)`.
    ///
    /// The two callers that map a point in anger — the warp and the composite — each destructure [`rows`](Self::rows)
    /// and expand the expression themselves, for the opposite reasons their own comments give: one must not hoist the
    /// row terms and the other must. This is the plain form, and what the tests that check the fit's own arithmetic
    /// read it through.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the two mapping loops expand the matrix for their own hoisting")
    )]
    pub(crate) fn apply(self, x: f32, y: f32) -> (f32, f32) {
        let [[a00, a01, a02], [a10, a11, a12]] = self.rows;

        (a00 * x + a01 * y + a02, a10 * x + a11 * y + a12)
    }

    /// This transform's inverse, or [`IDENTITY`](Self::IDENTITY) where it is singular.
    ///
    /// The identity rather than an error, which is the reference's answer: a singular transform can only come from
    /// landmarks that carry no orientation, and [`similarity`] has already answered that case with a translation —
    /// so what reaches here singular is a transform nothing can produce. Returning the identity means the aligned
    /// square is then a crop of the photograph's top-left corner, which is a wrong restoration; an error would be a
    /// refusal of a photograph, which is worse and which no caller could act on.
    pub(crate) fn inverse(self) -> Self {
        let [[a, b, c], [d, e, f]] = self.rows;

        let det = a * e - b * d;

        if det > -SINGULAR_DET && det < SINGULAR_DET {
            return Self::IDENTITY;
        }

        let inv_det = 1.0 / det;

        Self {
            rows: [
                [e * inv_det, -b * inv_det, (b * f - c * e) * inv_det],
                [-d * inv_det, a * inv_det, (c * d - a * f) * inv_det],
            ],
        }
    }
}

/// [`TEMPLATE`] scaled from [`TEMPLATE_SIZE`] to a `tile`-pixel square.
pub(crate) fn scaled_template(tile: u32) -> [Point; Face::LANDMARKS] {
    let scale = tile as f32 / TEMPLATE_SIZE;

    TEMPLATE.map(|point| Point::new(point.x * scale, point.y * scale))
}

/// The transform mapping `landmarks` onto [`TEMPLATE`] scaled to a `tile`-pixel square.
///
/// What a face's alignment is: the **forward** transform, from the photograph's coordinates into the aligned
/// square's. [`warp`](super::warp) samples through its [`inverse`](Affine::inverse) and
/// [`composite`](super::composite) maps through it as it stands, which is the one place the two directions have to
/// stay straight.
pub(crate) fn alignment(landmarks: &[Point; Face::LANDMARKS], tile: u32) -> Affine {
    similarity(landmarks, &scaled_template(tile))
}

/// The least-squares similarity transform taking `src` onto `dst`.
///
/// A **similarity** — one scale, one rotation, one translation — fitted over **all** the points rather than from any
/// subset of them. A face is rigid under a change of viewpoint to the accuracy this needs, and an unconstrained
/// affine fit would stretch it to satisfy landmarks the detector placed a pixel or two out. That is why the matrix
/// is built from two coefficients: `a` is `cos(θ) * scale` and `b` is `sin(θ) * scale`, and the rows spell them as
/// `[a, -b]` and `[b, a]`, which is a rotation and a uniform scale and admits no shear at all.
///
/// A covariance-based fit, transcribed from the reference's `calculateSimilarityTransform`: it minimises the sum of
/// squared distances between the transformed source points and the destination ones. The source's own `sXY`
/// cross-term that function accumulates is not read by it and is not accumulated here — it cannot be, since a
/// similarity has no coefficient for it.
///
/// Where the source points carry no extent at all — every landmark coincident, so no scale and no rotation can be
/// fit — the answer is the translation mapping the source's centroid onto the destination's, rather than the NaN
/// matrix a division by zero would produce.
fn similarity(src: &[Point; Face::LANDMARKS], dst: &[Point; Face::LANDMARKS]) -> Affine {
    let points = Face::LANDMARKS as f32;

    let (mut src_mean_x, mut src_mean_y) = (0.0_f32, 0.0_f32);
    let (mut dst_mean_x, mut dst_mean_y) = (0.0_f32, 0.0_f32);

    for index in 0..Face::LANDMARKS {
        src_mean_x += src[index].x;
        src_mean_y += src[index].y;
        dst_mean_x += dst[index].x;
        dst_mean_y += dst[index].y;
    }

    src_mean_x /= points;
    src_mean_y /= points;
    dst_mean_x /= points;
    dst_mean_y /= points;

    let (mut s_xx, mut s_yy) = (0.0_f32, 0.0_f32);
    let (mut dx_sx, mut dx_sy, mut dy_sx, mut dy_sy) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);

    for index in 0..Face::LANDMARKS {
        let sx = src[index].x - src_mean_x;
        let sy = src[index].y - src_mean_y;

        let dx = dst[index].x - dst_mean_x;
        let dy = dst[index].y - dst_mean_y;

        // The source's own variance, which is what the fit is normalised by.
        s_xx += sx * sx;
        s_yy += sy * sy;

        // The cross-covariance, which is what carries the rotation and the scale.
        dx_sx += dx * sx;
        dx_sy += dx * sy;
        dy_sx += dy * sx;
        dy_sy += dy * sy;
    }

    let src_norm = s_xx + s_yy;

    if src_norm < DEGENERATE_NORM {
        return Affine::translation(dst_mean_x - src_mean_x, dst_mean_y - src_mean_y);
    }

    // cos(θ) * scale and sin(θ) * scale, which between them are the whole of a similarity's linear part.
    let a = (dx_sx + dy_sy) / src_norm;
    let b = (dy_sx - dx_sy) / src_norm;

    let tx = dst_mean_x - (a * src_mean_x - b * src_mean_y);
    let ty = dst_mean_y - (b * src_mean_x + a * src_mean_y);

    Affine { rows: [[a, -b, tx], [b, a, ty]] }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// How close two coordinates have to be to count as the same point, in pixels.
    ///
    /// A hundredth of a pixel, which is the precision a [`Point`] is quantized to in the first place — so a fit that
    /// lands inside it has landed on the point the detector could have reported.
    const EPSILON: f32 = 0.01;

    /// Five landmarks placed by rotating and scaling the template about its own centre, so the fit has a known
    /// answer to recover.
    ///
    /// `degrees` anticlockwise in the usual screen sense, then `scale`, then a translation by `(dx, dy)`. Used
    /// wherever a test needs a face whose transform is known rather than a face whose transform is plausible.
    pub(crate) fn transformed_template(degrees: f32, scale: f32, dx: f32, dy: f32) -> [Point; Face::LANDMARKS] {
        let radians = degrees.to_radians();
        let (sin, cos) = (radians.sin(), radians.cos());

        // About the template's own centroid, so the rotation does not also translate the set.
        let centre_x = TEMPLATE.iter().map(|point| point.x).sum::<f32>() / Face::LANDMARKS as f32;
        let centre_y = TEMPLATE.iter().map(|point| point.y).sum::<f32>() / Face::LANDMARKS as f32;

        TEMPLATE.map(|point| {
            let (x, y) = (point.x - centre_x, point.y - centre_y);

            Point::new(scale * (cos * x - sin * y) + centre_x + dx, scale * (sin * x + cos * y) + centre_y + dy)
        })
    }

    #[test]
    fn the_template_is_the_references_five_literals_in_the_documented_landmark_order() {
        // Pinned as literals because they are the canonical position every face in every photograph is restored at:
        // a reordering here is a misalignment nothing reports, and a retyped coordinate is a face a few pixels off
        // centre in every result the application has ever produced.
        assert_eq!(TEMPLATE_SIZE, 512.0);
        assert_eq!(
            TEMPLATE,
            [
                Point::new(192.98, 239.95), // Left eye
                Point::new(318.90, 240.19), // Right eye
                Point::new(256.63, 314.02), // Nose
                Point::new(201.26, 371.41), // Left mouth corner
                Point::new(313.09, 371.15), // Right mouth corner
            ]
        );

        // The order stated as facts about the layout rather than as a comment beside it: the eyes are above the
        // mouth, the mouth corners straddle the nose, and each pair's left is left of its right. A transposition
        // that a coordinate-by-coordinate comparison would catch only if the literals changed fails here too.
        assert!(TEMPLATE[0].x < TEMPLATE[1].x, "the left eye is not left of the right eye");
        assert!(TEMPLATE[3].x < TEMPLATE[4].x, "the left mouth corner is not left of the right one");
        assert!(TEMPLATE[0].y < TEMPLATE[2].y, "the eyes are not above the nose");
        assert!(TEMPLATE[2].y < TEMPLATE[3].y, "the nose is not above the mouth");
    }

    #[test]
    fn the_template_is_scaled_into_whatever_square_a_model_accepts() {
        // At the shipping tile size the scale is exactly one, which is what makes the two numbers' independence
        // invisible today — and is why it is checked rather than assumed.
        assert_eq!(scaled_template(512), TEMPLATE);

        for (index, point) in scaled_template(256).iter().enumerate() {
            assert!((point.x - TEMPLATE[index].x / 2.0).abs() < EPSILON, "{index} was not halved on x");
            assert!((point.y - TEMPLATE[index].y / 2.0).abs() < EPSILON, "{index} was not halved on y");
        }
    }

    #[test]
    fn landmarks_already_at_the_templates_positions_fit_the_identity() {
        // The base case, and the one a whole-warp test rests on: a face already in the canonical position is aligned
        // by doing nothing to it, so an aligned square of it is a plain crop.
        let fitted = alignment(&TEMPLATE, 512);

        assert!((fitted.rows[0][0] - 1.0).abs() < EPSILON, "the scale was not one: {fitted:?}");
        assert!((fitted.rows[1][1] - 1.0).abs() < EPSILON, "the scale was not one: {fitted:?}");
        assert!(fitted.rows[0][1].abs() < EPSILON, "the fit rotated an unrotated face: {fitted:?}");
        assert!(fitted.rows[1][0].abs() < EPSILON, "the fit rotated an unrotated face: {fitted:?}");
        assert!(fitted.rows[0][2].abs() < EPSILON, "the fit translated a centred face: {fitted:?}");
        assert!(fitted.rows[1][2].abs() < EPSILON, "the fit translated a centred face: {fitted:?}");
    }

    #[test]
    fn a_known_rotation_and_uniform_scale_are_recovered() {
        // The property the alignment exists for: a face that is rotated and at the wrong size in the photograph is
        // handed to the model upright and at the template's size. Driven from a synthetic face whose transform is
        // known, so what is checked is the arithmetic rather than that some matrix came back.
        for (degrees, scale) in [(0.0, 1.0), (15.0, 1.0), (-30.0, 1.0), (0.0, 2.0), (45.0, 0.5), (-12.5, 1.75)] {
            let landmarks = transformed_template(degrees, scale, 37.5, -19.25);
            let fitted = alignment(&landmarks, 512);

            // The fit inverts what was applied, so its scale is the reciprocal and its rotation the negation.
            let radians = -degrees.to_radians();
            let expected_a = radians.cos() / scale;
            let expected_b = radians.sin() / scale;

            assert!(
                (fitted.rows[0][0] - expected_a).abs() < EPSILON,
                "{degrees} degrees at {scale}x did not recover the scale: {fitted:?}"
            );
            assert!(
                (fitted.rows[1][0] - expected_b).abs() < EPSILON,
                "{degrees} degrees at {scale}x did not recover the rotation: {fitted:?}"
            );

            // And the whole of it, checked where it matters: every landmark lands on its template point.
            let template = scaled_template(512);
            for (index, landmark) in landmarks.iter().enumerate() {
                let (x, y) = fitted.apply(landmark.x, landmark.y);

                assert!(
                    (x - template[index].x).abs() < EPSILON && (y - template[index].y).abs() < EPSILON,
                    "landmark {index} landed at ({x}, {y}) rather than {:?}",
                    template[index]
                );
            }
        }
    }

    #[test]
    fn a_shear_free_source_produces_no_shear() {
        // What makes this a similarity rather than an affine fit. An unconstrained fit over five landmarks the
        // detector placed a pixel or two out would stretch the face to satisfy them, which is a restoration of a
        // face nobody has. The matrix's own shape is the guarantee: `[a, -b]` over `[b, a]` has no coefficient a
        // shear could land in, so the check is that the two rows are that shape.
        for (degrees, scale) in [(0.0, 1.0), (23.0, 1.4), (-67.5, 0.8)] {
            let fitted = alignment(&transformed_template(degrees, scale, 0.0, 0.0), 512);
            let [[a, negated_b, _], [b, second_a, _]] = fitted.rows;

            assert!((a - second_a).abs() < EPSILON, "the two diagonal terms disagreed, which is a non-uniform scale");
            assert!(
                (negated_b + b).abs() < EPSILON,
                "the two off-diagonal terms are not negations, which is a shear"
            );
        }
    }

    #[test]
    fn coincident_landmarks_translate_rather_than_producing_nan() {
        // A face whose five landmarks are all the same point carries no orientation and no size, so there is nothing
        // to rotate or scale — and dividing by its zero variance would hand the warp a NaN matrix, which samples
        // nothing and produces a black square presented as a restoration.
        let coincident = [Point::new(120.0, 80.0); Face::LANDMARKS];
        let fitted = alignment(&coincident, 512);

        for row in fitted.rows {
            for coefficient in row {
                assert!(coefficient.is_finite(), "the degenerate fit produced {coefficient}: {fitted:?}");
            }
        }

        assert_eq!(fitted.rows[0][0], 1.0, "the degenerate fit is not a pure translation: {fitted:?}");
        assert_eq!(fitted.rows[1][1], 1.0, "the degenerate fit is not a pure translation: {fitted:?}");
        assert_eq!(fitted.rows[0][1], 0.0, "the degenerate fit is not a pure translation: {fitted:?}");
        assert_eq!(fitted.rows[1][0], 0.0, "the degenerate fit is not a pure translation: {fitted:?}");

        // And it is the translation that maps the landmarks' centre onto the template's, which is the only
        // placement available: the point becomes the template's centroid.
        let template = scaled_template(512);
        let centre_x = template.iter().map(|point| point.x).sum::<f32>() / Face::LANDMARKS as f32;
        let centre_y = template.iter().map(|point| point.y).sum::<f32>() / Face::LANDMARKS as f32;

        let (x, y) = fitted.apply(120.0, 80.0);
        assert!(
            (x - centre_x).abs() < EPSILON && (y - centre_y).abs() < EPSILON,
            "({x}, {y}) is not the centroid"
        );
    }

    #[test]
    fn a_point_round_trips_through_a_transform_and_its_inverse() {
        // The pairing the warp and the composite rest on: the warp samples the photograph through the inverse and
        // the composite maps the photograph through the forward transform, so the two directions have to be exact
        // inverses or a restored face lands beside the face it restored.
        let fitted = alignment(&transformed_template(31.5, 1.3, 220.0, 140.0), 512);
        let back = fitted.inverse();

        for (x, y) in [(0.0, 0.0), (511.0, 0.0), (0.0, 511.0), (511.0, 511.0), (256.5, 133.25), (-40.0, 900.0)] {
            let (forward_x, forward_y) = back.apply(x, y);
            let (round_x, round_y) = fitted.apply(forward_x, forward_y);

            assert!(
                (round_x - x).abs() < EPSILON && (round_y - y).abs() < EPSILON,
                "({x}, {y}) round-tripped to ({round_x}, {round_y})"
            );
        }
    }

    #[test]
    fn a_singular_transform_inverts_to_the_identity_rather_than_to_infinities() {
        // Unreachable from a fitted alignment — the degenerate case is answered with a translation above — and
        // guarded anyway, because what it prevents is a matrix of infinities that samples every destination pixel
        // from the same source coordinate.
        let collapsed = Affine { rows: [[0.0, 0.0, 10.0], [0.0, 0.0, 20.0]] };

        assert_eq!(collapsed.inverse(), Affine::IDENTITY);
        assert_eq!(Affine::IDENTITY.inverse(), Affine::IDENTITY);

        // A translation is not singular, so it inverts to the opposite translation rather than being caught by the
        // guard above.
        assert_eq!(Affine::translation(7.0, -3.0).inverse(), Affine::translation(-7.0, 3.0));
    }

    #[test]
    fn the_identity_leaves_a_point_where_it_is() {
        for (x, y) in [(0.0, 0.0), (511.0, 511.0), (123.25, -4.5)] {
            assert_eq!(Affine::IDENTITY.apply(x, y), (x, y));
        }
    }
}
