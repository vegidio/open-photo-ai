//! The identity a result carries, and the key a result that is not a picture is stored under.

use super::*;
use crate::inference::process::identity::DATA_DISCRIMINATOR;

#[test]
fn a_chained_pair_of_runs_agrees_with_one_run_over_the_concatenated_operations() {
    // The reference implementation's own bug, fixed here rather than left for the cache slice: a result fed back
    // in has to land on exactly the identity its pixels were derived under.
    let input = "cafebabecafebabe";
    let (first, second) = (kyoto(2.0), kyoto(4.0));

    for depth in [ChannelDepth::Eight, ChannelDepth::Sixteen] {
        let chained = identity_after(
            &identity_after(input, std::slice::from_ref(&first), depth),
            std::slice::from_ref(&second),
            depth,
        );
        let together = identity_after(input, &[first.clone(), second.clone()], depth);

        assert_eq!(chained, together, "the chained pair disagreed at {depth:?}");
    }
}

#[test]
fn a_result_never_carries_the_identity_of_what_it_was_computed_from() {
    let input = "cafebabecafebabe";

    let once = identity_after(input, &[kyoto(2.0)], ChannelDepth::Eight);
    let twice = identity_after(input, &[kyoto(2.0), kyoto(4.0)], ChannelDepth::Eight);

    assert_ne!(once, input, "the result carried the input's own identity");
    assert_ne!(twice, input);
    // And a chained pair is not mistaken for the first run alone, which is the other half of the same defect.
    assert_ne!(once, twice);
}

#[test]
fn two_runs_differing_only_in_which_operation_was_applied_are_distinguishable() {
    let input = "cafebabecafebabe";

    assert_ne!(
        identity_after(input, &[kyoto(2.0)], ChannelDepth::Eight),
        identity_after(input, &[kyoto(4.0)], ChannelDepth::Eight)
    );
}

#[test]
fn two_runs_differing_only_in_depth_are_distinguishable() {
    // The defect D2 fixes. The same chain at 8 and at 16 bits produces two images that are not interchangeable,
    // so anything keyed on the identity would otherwise serve one where the other was asked for.
    let input = "cafebabecafebabe";

    assert_ne!(
        identity_after(input, &[kyoto(2.0)], ChannelDepth::Eight),
        identity_after(input, &[kyoto(2.0)], ChannelDepth::Sixteen)
    );
}

#[tokio::test]
async fn two_ways_of_asking_for_one_depth_produce_one_identity() {
    // It is the *resolved* depth that is folded, so `Source` over a 16-bit photograph and `Sixteen` are the same
    // request: the same pixels have to carry the same name, or the two runs never share an entry.
    let backend = Fake::new();
    let sixteen = Picture::new(
        "/pictures/raw.dng",
        Arc::new(DynamicImage::ImageRgb16(ImageBuffer::new(300, 200))),
        "cafebabecafebabe",
    );

    let stated = ProcessOptions { depth: OutputDepth::Sixteen, ..Default::default() };
    let inherited = ProcessOptions { depth: OutputDepth::Source, ..Default::default() };

    let stated = process(&backend, None, &sixteen, &[kyoto(2.0)], Some(stated)).await.unwrap().picture;
    let inherited = process(&backend, None, &sixteen, &[kyoto(2.0)], Some(inherited)).await.unwrap().picture;

    assert_eq!(stated.identity(), inherited.identity());

    // And the other direction stays distinguishable: an 8-bit source asked for `Source` is not the 16-bit run.
    let eight = process(&backend, None, &picture(300, 200), &[kyoto(2.0)], None).await.unwrap().picture;
    assert_ne!(eight.identity(), stated.identity());
}

#[test]
fn a_chained_pair_at_two_depths_does_not_collide_with_one_at_the_second_alone() {
    // The counterexample D2 is written against, and the whole reason the depth is folded per operation rather
    // than appended once to the finished fold. `[a]` at 8 then `[b]` at 16 ran its second operation over an
    // 8-bit intermediate; `[a]` at 16 then `[b]` at 16 did not, and the two are different pictures.
    let input = "cafebabecafebabe";
    let (a, b) = (kyoto(2.0), kyoto(4.0));

    let mixed = identity_after(
        &identity_after(input, std::slice::from_ref(&a), ChannelDepth::Eight),
        std::slice::from_ref(&b),
        ChannelDepth::Sixteen,
    );
    let throughout = identity_after(
        &identity_after(input, std::slice::from_ref(&a), ChannelDepth::Sixteen),
        std::slice::from_ref(&b),
        ChannelDepth::Sixteen,
    );

    assert_ne!(mixed, throughout);
}

#[test]
fn two_runs_of_one_operation_over_different_images_are_distinguishable() {
    assert_ne!(
        identity_after("cafebabecafebabe", &[kyoto(2.0)], ChannelDepth::Eight),
        identity_after("f00df00df00df00d", &[kyoto(2.0)], ChannelDepth::Eight)
    );
}

#[test]
fn an_empty_chain_leaves_the_identity_untouched_whatever_depth_was_asked_for() {
    // Applying nothing derives nothing, so there is nothing to name — and a depth nothing was produced at cannot
    // give it something to name either.
    for depth in [ChannelDepth::Eight, ChannelDepth::Sixteen] {
        assert_eq!(identity_after("cafebabecafebabe", &[], depth), "cafebabecafebabe");
    }
}

#[test]
fn the_composed_identity_is_the_same_fixed_width_however_long_the_chain() {
    let input = "cafebabecafebabe";
    let long: Vec<Operation> = (0..8).map(|_| kyoto(2.0)).collect();
    let depth = ChannelDepth::Eight;

    assert_eq!(identity_after(input, &long, depth).len(), identity_after(input, &[kyoto(2.0)], depth).len());
}

/// A detection operation, which is what the data path actually runs — the one family whose result is not a
/// picture — as the subject a report and a key name it by.
fn newyork() -> crate::models::Subject {
    crate::models::Subject::Analysis(crate::models::Detection::newyork(FloatPrecision::Fp32))
}

#[test]
fn a_data_key_is_the_same_string_every_time_it_is_derived() {
    // The read and the write call this separately, so a derivation that varied between two calls would store
    // every result under a slot nothing ever looks in — a store that is written and never read.
    let input = "cafebabecafebabe";

    assert_eq!(identity_of_data(input, &newyork()), identity_of_data(input, &newyork()));
}

#[test]
fn a_data_key_separates_the_image_and_the_operation_that_was_run_over_it() {
    // Both halves of "stored under the image together with the operation": neither an analysis of another
    // photograph nor another analysis of this one may be served in its place.
    let input = "cafebabecafebabe";

    assert_ne!(identity_of_data(input, &newyork()), identity_of_data("f00df00df00df00d", &newyork()));
    assert_ne!(identity_of_data(input, &newyork()), identity_of_data(input, &as_subject(kyoto(2.0))));
}

#[test]
fn a_data_key_and_a_chain_key_are_never_the_same_string() {
    // The one that matters, and the reason this derivation sits beside `identity_after` rather than in the driver
    // that calls it: a collision would serve a stored picture where a result belongs. The encodings would catch
    // it — JSON bytes are not a PNG — so it would cost the hit rather than give a wrong answer, but silently and
    // for every affected run. An edit that dropped the discriminator has to fail here.
    //
    // The **image** operation is the case that would collide, which is why it is put through both derivations
    // although only the data path calls the first: a detection has no chain form at all, so for it the two
    // derivations have no shared input to collide over. Both are checked anyway, and at **both** depths, because
    // the discriminator's whole job is to be a value no depth tag can be.
    let image = kyoto(2.0);

    for identity in ["cafebabecafebabe", "f00df00df00df00d"] {
        for subject in [newyork(), as_subject(image.clone())] {
            let data = identity_of_data(identity, &subject);

            for depth in [ChannelDepth::Eight, ChannelDepth::Sixteen] {
                assert_ne!(
                    data,
                    identity_after(identity, std::slice::from_ref(&image), depth),
                    "a data key collided with a chain key at {}",
                    depth.tag()
                );
            }
        }
    }

    // And the discriminator is not something a depth could spell, which is what makes the loop above a property
    // rather than a sample of two depths.
    assert!(
        ![ChannelDepth::Eight.tag(), ChannelDepth::Sixteen.tag()].contains(&DATA_DISCRIMINATOR),
        "the data discriminator is a depth tag"
    );
}
