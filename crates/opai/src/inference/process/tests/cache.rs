//! Serving a chain from the run cache, and what a run reports when it is.

use super::*;

#[tokio::test]
async fn a_result_computed_on_one_provider_is_served_to_a_request_for_another() {
    // The execution provider is deliberately not part of what identifies an entry, and this is what that means
    // for a user: the same enhancement is the same enhancement whichever processor produced it. It holds by
    // construction — `identity_after` never sees a provider — and is pinned here because nothing else would
    // notice a later change that folded one in, which would quietly halve the hit rate of every mixed session.
    // A caller that needs each provider to actually run turns the cache off for that run instead.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);
    let on = |provider| Some(ProcessOptions { provider, ..Default::default() });

    let computed = process(&backend, Some(&cache), &source, &[kyoto(2.0)], on(ExecutionProvider::Cpu))
        .await
        .unwrap()
        .picture;
    let acquired = backend.log().acquired.len();
    let ran = backend.log().ran.len();

    let served = process(&backend, Some(&cache), &source, &[kyoto(2.0)], on(ExecutionProvider::CoreMl))
        .await
        .unwrap()
        .picture;

    assert_eq!(backend.log().acquired.len(), acquired, "a differing provider acquired a session of its own");
    assert_eq!(backend.log().ran.len(), ran, "a differing provider ran the model again");
    assert_eq!(served.identity(), computed.identity());
    assert_eq!(served.pixels().as_bytes(), computed.pixels().as_bytes());
}

#[tokio::test]
async fn a_chain_extended_by_one_operation_runs_only_the_new_one() {
    // The per-*step* keying, which is what makes dragging a slider cheap: the first two operations are served from
    // what the earlier run stored, and only the third is computed.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);
    let two = [kyoto(2.0), kyoto(4.0)];
    let three = [kyoto(2.0), kyoto(4.0), kyoto(2.0)];

    cached_run(&backend, Some(&cache), &source, &two, OutputDepth::Eight).await.unwrap();
    let after_two = backend.log().passes();

    cached_run(&backend, Some(&cache), &source, &three, OutputDepth::Eight).await.unwrap();

    // Exactly one more pass than the first run, and it is the third operation's.
    let after_three = backend.log().passes();
    assert_eq!(after_two, vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);
    assert_eq!(after_three, vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16", "up_kyoto_2x_fp16"]);
}

#[tokio::test]
async fn the_same_operation_after_a_different_predecessor_is_a_different_entry() {
    // The fold, seen from the run: `[a, c]` and `[b, c]` end in the same operation over different pixels, so the
    // second chain's last step cannot be served what the first chain's stored.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);

    cached_run(&backend, Some(&cache), &source, &[kyoto(2.0), kyoto(2.0)], OutputDepth::Eight)
        .await
        .unwrap();
    let after_first = backend.log().ran.len();

    let second = cached_run(&backend, Some(&cache), &source, &[kyoto(4.0), kyoto(2.0)], OutputDepth::Eight)
        .await
        .unwrap();

    assert!(backend.log().ran.len() > after_first, "the second chain was served the first chain's last step");
    // 120x80 quadrupled and then doubled, which only running both reaches.
    assert_eq!(second.dimensions(), (960, 640));
}

#[tokio::test]
async fn two_runs_differing_only_in_depth_do_not_share_an_entry() {
    // The defect D2 fixes, as the failure it would actually produce: a caller asking for 16 bits handed the 8-bit
    // result of an earlier run, with nothing to tell it apart from a computed one.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);

    cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();
    let sixteen = cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Sixteen).await.unwrap();

    assert!(
        matches!(sixteen.pixels(), DynamicImage::ImageRgb16(_)),
        "the 16-bit request was served the 8-bit run: {:?}",
        sixteen.pixels()
    );
    assert_eq!(backend.log().ran.len() % 2, 0, "the second run did not cover the same tiles as the first");
}

#[tokio::test]
async fn a_run_with_the_cache_turned_off_computes_everything_and_stores_nothing() {
    // Both halves of the bypass: it reads nothing, and — the half a caller that must not persist its images
    // depends on — it leaves nothing behind for anything else to read either.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);
    let bypass = || Some(ProcessOptions { cache: false, ..Default::default() });

    // Stored by a cached run first, so that "it read nothing" is about a store that actually had the answer.
    let expected = cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();
    let after_warm = backend.log().ran.len();

    let bypassed = process(&backend, Some(&cache), &source, &[kyoto(2.0)], bypass()).await.unwrap().picture;

    assert!(backend.log().ran.len() > after_warm, "a bypassed run was served from the cache");
    // And it returns exactly what the cached run would have: turning the cache off changes what it costs, never
    // what it produces.
    assert_eq!(bypassed.identity(), expected.identity());
    assert_eq!(bypassed.pixels().as_bytes(), expected.pixels().as_bytes());

    // Nothing of its own was stored: a second chain it alone computed finds nothing to serve.
    //
    // Against a store of its own rather than the one above, and that is what makes the assertion evidence. The
    // store above already holds `[2x]` from the warm-up, so a bypass that leaked its *first* step would write an
    // entry that was there anyway and the later run would be unable to tell — it would recompute `[4x]` either
    // way. An untouched store makes every entry the later run finds attributable to the bypass alone.
    let untouched = store();
    let fresh = Fake::new();
    process(&fresh, Some(&untouched), &source, &[kyoto(2.0), kyoto(4.0)], bypass()).await.unwrap();

    let reader = Fake::new();
    cached_run(&reader, Some(&untouched), &source, &[kyoto(2.0), kyoto(4.0)], OutputDepth::Eight)
        .await
        .unwrap();
    assert_eq!(
        reader.log().passes(),
        vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"],
        "a bypassed run stored a step, so a later cached run was served it"
    );
}

#[tokio::test]
async fn a_run_that_reads_from_the_cache_cannot_decline_to_write_back_to_it() {
    // Decided once for the run rather than per operation. Observed as the property that follows from it: a run
    // that was served its first operation still stored its second, so the chain is not left half-recorded.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);

    cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();
    // Reads the first operation and computes the second.
    cached_run(&backend, Some(&cache), &source, &[kyoto(2.0), kyoto(4.0)], OutputDepth::Eight)
        .await
        .unwrap();

    let reader = Fake::new();
    cached_run(&reader, Some(&cache), &source, &[kyoto(2.0), kyoto(4.0)], OutputDepth::Eight)
        .await
        .unwrap();

    assert!(
        reader.log().ran.is_empty(),
        "the operation computed by a partly-served run was not written back"
    );
    assert!(reader.log().acquired.is_empty());
}

#[tokio::test]
async fn a_write_that_fails_does_not_fail_the_run_that_produced_it() {
    // A store that declines everything, which is what a full or read-only disk amounts to from up here: the
    // budget is smaller than any result, so every write comes back `NotAdmitted`.
    let backend = Fake::new();
    let refuses = Memo::memory(rust_sak::memo::CacheOpts::new().max_capacity(64)).unwrap();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0)];

    let produced = cached_run(&backend, Some(&refuses), &source, &chain, OutputDepth::Eight).await.unwrap();

    // The run returned its result, and the operations after the failed write still ran: 120x80 doubled and then
    // quadrupled is only reachable by running both.
    assert_eq!(produced.dimensions(), (960, 640));
    assert_eq!(backend.log().passes(), vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);

    // Nothing was kept, which is what makes this a dropped write rather than a successful one — and the cost is a
    // recompute and nothing else.
    let again = Fake::new();
    cached_run(&again, Some(&refuses), &source, &chain, OutputDepth::Eight).await.unwrap();
    assert_eq!(again.log().passes(), vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);
}

#[tokio::test]
async fn a_run_whose_store_serves_nothing_and_keeps_nothing_behaves_as_though_there_were_none() {
    // The `CacheMode::None` arm as a run sees it: `process` is handed no store at all, and produces exactly what
    // a cached run produces.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);

    let cached = cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();
    let uncached = cached_run(&Fake::new(), None, &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();

    assert_eq!(uncached.identity(), cached.identity());
    assert_eq!(uncached.pixels().as_bytes(), cached.pixels().as_bytes());
}

#[tokio::test]
async fn a_chain_stored_end_to_end_decodes_one_entry_rather_than_every_prefix() {
    // The case the cache exists for — a slider dragged on the last operation of a chain — and the reason the walk
    // reads bytes rather than pictures. Each prefix it passes is superseded by the next, so decoding them on the
    // way past is a several-hundred-megabyte PNG thrown away per step.
    let cache = store();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(2.0), kyoto(2.0)];

    // Fill every prefix.
    let stored = cached_run(&Fake::new(), Some(&cache), &source, &chain, OutputDepth::Eight).await.unwrap();

    // Every entry the run above wrote is a prefix of this chain, so the walk passes all three.
    let keys: Vec<String> = (0..chain.len())
        .map(|end| identity_after(source.identity(), &chain[..=end], ChannelDepth::Eight))
        .collect();
    for key in &keys {
        assert!(cache.get_bytes(key).unwrap().is_some(), "the first run stored no entry for a prefix");
    }

    // Corrupting every prefix *but the last* changes nothing: a run that decoded them would fail to, and would
    // then have to re-run the operations it had skipped.
    for key in &keys[..keys.len() - 1] {
        cache.set_bytes(key, b"\x89PNG\r\n\x1a\n and then nothing", ENTRY_TTL).unwrap();
    }

    let backend = Fake::new();
    let served = cached_run(&backend, Some(&cache), &source, &chain, OutputDepth::Eight).await.unwrap();

    assert!(backend.log().passes().is_empty(), "an operation ran despite the whole chain being stored");
    assert_eq!(served.pixels().as_bytes(), stored.pixels().as_bytes());
}

#[tokio::test]
async fn a_stored_prefix_that_will_not_decode_runs_the_operations_it_had_skipped() {
    // The fourth way a read misses, now that the decode happens after the walk rather than during it: bytes that
    // are present but no longer an image. The response has to be the response to every other miss — run the
    // operation — which means the run of hits skipped on their account is walked again.
    let cache = store();
    let source = picture(120, 80);
    // Distinct scales so the two runs are distinguishable: `Log::passes` collapses consecutive identical
    // artifacts, which is what makes a repeated pass within one operation read as one.
    let chain = [kyoto(2.0), kyoto(4.0)];

    let expected = cached_run(&Fake::new(), Some(&cache), &source, &chain, OutputDepth::Eight).await.unwrap();

    // The *last* prefix is the corrupt one, so the walk skips both operations and then finds it will not decode.
    let last = identity_after(source.identity(), &chain, ChannelDepth::Eight);
    cache.set_bytes(&last, b"\x89PNG\r\n\x1a\n and then nothing", ENTRY_TTL).unwrap();

    let backend = Fake::new();
    let served = cached_run(&backend, Some(&cache), &source, &chain, OutputDepth::Eight).await.unwrap();

    // Both ran: the first prefix is still intact, but it is the *input* to the second operation and the entry
    // that would have stood in for both is gone — so neither may be skipped on its account.
    assert_eq!(backend.log().passes(), vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);
    assert_eq!(
        served.pixels().as_bytes(),
        expected.pixels().as_bytes(),
        "the recompute produced a different image"
    );
}

/// Runs `chain` over a fresh backend against `cache`, collecting every report the caller received.
async fn reporting(cache: &Memo, source: &Picture, chain: &[Operation]) -> Vec<InferenceProgress> {
    let (reports, options) = recording();

    process(&Fake::new(), Some(cache), source, chain, Some(options))
        .await
        .expect("the chain must run");

    let collected = reports.lock().unwrap();
    collected.clone()
}

#[tokio::test]
async fn a_hit_reports_that_the_result_was_already_known_and_names_the_operation() {
    let cache = store();
    let source = picture(120, 80);

    cached_run(&Fake::new(), Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight)
        .await
        .unwrap();
    let reports = reporting(&cache, &source, &[kyoto(2.0)]).await;

    let cached: Vec<&InferenceProgress> =
        reports.iter().filter(|report| matches!(report.stage, Stage::Cached)).collect();

    assert_eq!(cached.len(), 1, "a served operation reported {} times", cached.len());
    assert_eq!(*cached[0].subject, as_subject(kyoto(2.0)), "the report did not name the operation it was for");
    // Its full share, so a front end can draw the bar rather than watching it stall.
    assert_eq!(cached[0].operation_fraction, 1.0);
    // And nothing claimed a transfer or a run that never happened.
    assert!(!reports.iter().any(|report| matches!(report.stage, Stage::Running | Stage::Installing(_))));
}

#[tokio::test]
async fn the_bar_never_decreases_across_a_mixture_of_served_and_computed_operations() {
    let cache = store();
    let source = picture(120, 80);

    // The first operation stored, the second not, so the run reports a hit handing over to a real run.
    cached_run(&Fake::new(), Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight)
        .await
        .unwrap();
    let reports = reporting(&cache, &source, &[kyoto(2.0), kyoto(4.0)]).await;

    assert!(reports.iter().any(|report| matches!(report.stage, Stage::Cached)));
    assert!(reports.iter().any(|report| matches!(report.stage, Stage::Running)));

    assert_never_decreases(&reports, "over the cached prefix");
    // The hit hands over at exactly its share rather than sending the bar back to the start of the next operation.
    let handover = reports.iter().position(|report| matches!(report.stage, Stage::Running)).unwrap();
    assert!((reports[handover - 1].chain_fraction - 0.5).abs() < 1e-9);
    assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0));
}

#[tokio::test]
async fn a_chain_every_operation_of_which_was_served_still_ends_on_exactly_one() {
    // The requirement the reference calls out and the reason its bar works: a skipped operation reports through
    // the same path as every other, so a run with nothing to do still fills the bar rather than never reporting.
    let cache = store();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0), kyoto(2.0)];

    cached_run(&Fake::new(), Some(&cache), &source, &chain, OutputDepth::Eight).await.unwrap();
    let reports = reporting(&cache, &source, &chain).await;

    assert_eq!(reports.len(), chain.len(), "a fully served chain reported {} times", reports.len());
    assert!(reports.iter().all(|report| matches!(report.stage, Stage::Cached)));
    assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0), "the bar was left short of 1");
}

#[tokio::test]
async fn an_empty_chain_returns_a_picture_identical_to_the_source() {
    let backend = Fake::new();
    let source = picture(300, 200);

    let result = process(&backend, None, &source, &[], None).await.unwrap().picture;

    assert_eq!(result.identity(), source.identity(), "applying nothing derived a new identity");
    assert_eq!(result.path(), source.path());
    assert!(std::ptr::eq(result.pixels(), source.pixels()), "applying nothing produced a second buffer");
}
