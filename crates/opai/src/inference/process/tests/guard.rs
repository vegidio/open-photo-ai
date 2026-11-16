//! What a run reports about a guarded model that kept part of the photograph unprocessed, computed and served alike.

use super::*;
use crate::inference::process::identity::{DATA_DISCRIMINATOR, GUARD_DISCRIMINATOR};
use crate::models::{Denoise, Strength};

/// Stockholm at full strength: the one denoiser whose guard has a rescue, so a tile kept by it was tried twice.
fn stockholm() -> Operation {
    Denoise::stockholm(FloatPrecision::Fp32, Strength::new(1.0).expect("a strength in range"))
}

/// A fake whose Stockholm blows up on every tile, retry included: the photograph its guard keeps whole.
fn exploding() -> Fake {
    let mut backend = Fake::new();
    backend.explodes = Some(stockholm().required_artifacts()[0].as_str().to_string());
    backend
}

#[tokio::test]
async fn a_guard_that_kept_every_tile_is_reported_beside_the_unchanged_photograph() {
    // The case this report exists for: the run succeeds, the result is the photograph it was given, and without the
    // report a user is left comparing two identical images. 600x400 is six tiles.
    let backend = exploding();
    let source = picture(600, 400);

    let enhanced = cached_enhancement(&backend, None, &source, &[stockholm()], OutputDepth::Eight).await.unwrap();

    assert_eq!(
        enhanced.picture.pixels().as_bytes(),
        source.pixels().as_bytes(),
        "a kept photograph was changed"
    );
    assert_eq!(
        enhanced.guards,
        vec![GuardReport { operation: stockholm(), tally: GuardTally { tiles: 6, kept: 6, rescued: 0 } }]
    );
    assert_eq!(enhanced.guards[0].tally.kept_share(), 1.0);
}

#[tokio::test]
async fn a_guard_that_kept_nothing_is_still_reported_and_an_unguarded_model_is_not() {
    // Two answers rather than one empty list: "the guard looked and every tile was sound" is not "nothing was
    // guarded".
    let backend = Fake::new();
    let source = picture(600, 400);

    let guarded = cached_enhancement(&backend, None, &source, &[stockholm()], OutputDepth::Eight).await.unwrap();
    assert_eq!(guarded.guards.len(), 1);
    assert_eq!(guarded.guards[0].tally, GuardTally { tiles: 6, kept: 0, rescued: 0 });

    let unguarded = cached_enhancement(&backend, None, &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();
    assert!(unguarded.guards.is_empty(), "an upscale reported a guard: {:?}", unguarded.guards);
}

#[tokio::test]
async fn a_served_run_reports_what_the_run_that_computed_it_did() {
    // Otherwise the warning is there on the first run and silently gone from the second, which is the run a user
    // repeating an enhancement to see why nothing changed is looking at.
    let cache = store();
    let source = picture(600, 400);

    let computed = cached_enhancement(&exploding(), Some(&cache), &source, &[stockholm()], OutputDepth::Eight)
        .await
        .unwrap();
    settled();

    let reader = exploding();
    let served = cached_enhancement(&reader, Some(&cache), &source, &[stockholm()], OutputDepth::Eight)
        .await
        .unwrap();

    assert!(reader.log().ran.is_empty(), "the second run executed the model rather than being served");
    assert!(reader.log().acquired.is_empty(), "the second run opened a session");
    assert_eq!(served.guards, computed.guards, "the served run reported something else");
    assert_eq!(served.guards[0].tally.kept, 6);
}

#[tokio::test]
async fn a_served_prefix_reports_its_guard_beside_the_step_computed_after_it() {
    // The forward walk serves Stockholm and computes the upscale after it: the report still covers the whole chain.
    let cache = store();
    let source = picture(120, 80);

    cached_enhancement(&exploding(), Some(&cache), &source, &[stockholm()], OutputDepth::Eight)
        .await
        .unwrap();
    settled();

    let reader = exploding();
    let extended = cached_enhancement(&reader, Some(&cache), &source, &[stockholm(), kyoto(2.0)], OutputDepth::Eight)
        .await
        .unwrap();

    assert_eq!(reader.log().passes(), vec!["up_kyoto_2x_fp16"], "the stored step was not served");
    assert_eq!(extended.guards.len(), 1);
    assert_eq!(extended.guards[0].operation, stockholm());
    assert_eq!(extended.guards[0].tally.kept, extended.guards[0].tally.tiles);
}

#[tokio::test]
async fn a_stored_picture_whose_tally_is_missing_is_computed_again() {
    // What an entry written before tallies were kept looks like: the picture under the step's key and nothing
    // beside it. Serving it would report no guard for a model that has one, so it is a miss instead — which is also
    // why no version tag retires those entries.
    let cache = store();
    let source = picture(600, 400);
    let key = identity_after(source.identity(), &[stockholm()], ChannelDepth::Eight);
    RunCache::new(cache.clone(), source.identity()).put(&key, source.pixels());

    let backend = exploding();
    let enhanced = cached_enhancement(&backend, Some(&cache), &source, &[stockholm()], OutputDepth::Eight)
        .await
        .unwrap();

    assert!(!backend.log().ran.is_empty(), "a picture with no tally was served");
    assert_eq!(enhanced.guards[0].tally, GuardTally { tiles: 6, kept: 6, rescued: 0 });
}

#[tokio::test]
async fn a_kept_tile_is_one_warning_naming_the_operation_and_the_counts_computed_or_served() {
    let cache = store();
    let source = picture(600, 400);

    for pass in ["computed", "served"] {
        let backend = exploding();
        let (log, outcome) = logging::records_of("info", || async {
            cached_enhancement(&backend, Some(&cache), &source, &[stockholm()], OutputDepth::Eight).await
        })
        .await;
        outcome.expect("a run");
        settled();

        let warned = records(&log, "a guarded model kept part of the photograph unprocessed");
        assert_eq!(warned.len(), 1, "{pass}: {log}");
        assert!(warned[0].contains("level=WARN"), "{pass}: {}", warned[0]);
        assert_eq!(field(warned[0], "operation"), Some(stockholm().cache_tag().as_str()), "{pass}");
        assert_eq!(field(warned[0], "kept"), Some("6"), "{pass}");
        assert_eq!(field(warned[0], "rescued"), Some("0"), "{pass}");
        assert_eq!(field(warned[0], "tiles"), Some("6"), "{pass}");
    }

    // And a guard that kept nothing writes nothing above `debug`.
    let (log, outcome) = logging::records_of("info", || async {
        cached_enhancement(&Fake::new(), None, &source, &[stockholm()], OutputDepth::Eight).await
    })
    .await;
    outcome.expect("a run");
    assert!(records(&log, "a guarded model kept part of the photograph unprocessed").is_empty(), "{log}");
}

#[test]
fn a_tally_is_stored_under_a_key_no_picture_or_data_result_can_have() {
    // Folded from the step's own key, so it is that step's and no other's, and ending in a word that is neither a
    // depth tag nor the data discriminator, so no chain key or data key can be folded to it.
    let step = identity_after("cafebabecafebabe", &[stockholm()], ChannelDepth::Eight);

    assert_ne!(identity_of_guard(&step), step);
    assert_ne!(
        identity_of_guard(&step),
        identity_of_guard(&identity_after("cafebabecafebabe", &[stockholm()], ChannelDepth::Sixteen))
    );
    assert!(
        ![ChannelDepth::Eight.tag(), ChannelDepth::Sixteen.tag(), DATA_DISCRIMINATOR].contains(&GUARD_DISCRIMINATOR),
        "the guard discriminator is a depth tag or the data discriminator"
    );
}
