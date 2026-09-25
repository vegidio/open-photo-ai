//! Carrying a planned chain out: order, failures, cancellation and depth.

use super::*;

#[tokio::test]
async fn a_chain_applies_its_operations_in_order() {
    // Two 2x upscales rather than one 4x: the order is then visible in *which* weights ran and in what the
    // dimensions did, not merely in how many tiles were covered.
    let backend = Fake::new();
    let chain = [kyoto(2.0), kyoto(4.0)];

    let result = run_with(&backend, source(300, 200), &chain).await.unwrap();

    assert_eq!(backend.log().passes(), vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);
    // 300x200 doubled and then quadrupled.
    assert_eq!(result.dimensions(), (2400, 1600));
}

#[tokio::test]
async fn an_eight_times_request_runs_its_two_passes_against_their_own_weights() {
    let backend = Fake::new();

    let result = run_with(&backend, source(300, 200), &[kyoto(8.0)]).await.unwrap();

    assert_eq!(backend.log().acquired, vec!["up_kyoto_4x_fp16", "up_kyoto_2x_fp16"]);
    assert_eq!(backend.log().passes(), vec!["up_kyoto_4x_fp16", "up_kyoto_2x_fp16"]);
    assert_eq!(result.dimensions(), (2400, 1600));
}

#[tokio::test]
async fn an_empty_chain_returns_the_source_unchanged() {
    // Applying nothing is a meaningful request — it is what a user who has toggled every enhancement off sends —
    // and it derives nothing, so the very pixels that went in come back.
    let backend = Fake::new();
    let input = source(300, 200);

    let result = run_with(&backend, Arc::clone(&input), &[]).await.unwrap();

    assert!(Arc::ptr_eq(&input, &result), "an empty chain produced a second image");
    assert!(backend.log().acquired.is_empty(), "an empty chain opened a model");
}

#[tokio::test]
async fn a_failure_partway_through_a_chain_returns_no_image() {
    let backend = Fake { fails_to_run: Some("up_kyoto_4x_fp16".to_string()), ..Fake::new() };

    let outcome = run_with(&backend, source(300, 200), &[kyoto(2.0), kyoto(4.0)]).await;

    let Err(InferenceError::Run { operation, .. }) = outcome else {
        panic!("a failing second operation returned {outcome:?}");
    };
    // Not the first operation's result, which had already succeeded: a caller handed that would show a user a 2x
    // image as though it were the 4x one they asked for.
    assert_eq!(operation, kyoto(4.0).display_name());
    assert_eq!(backend.log().passes(), vec!["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);
}

#[tokio::test]
async fn a_model_that_cannot_be_put_on_disk_fails_the_run_distinguishably_from_one_that_would_not_open() {
    // The two halves of a session failure, driven through the chain rather than only through the conversion: a
    // front end retries one and reports the other as a broken install, so the run they produce must not be the
    // same error. Both take the one `map_err` in `run`, which is what makes driving both worth the lines.
    let cannot_install = Fake { fails_to_install: Some("up_kyoto_2x_fp16".to_string()), ..Fake::new() };
    let cannot_open = Fake { fails_to_open: Some("up_kyoto_2x_fp16".to_string()), ..Fake::new() };

    let installing = run_with(&cannot_install, source(300, 200), &[kyoto(2.0)]).await;
    let opening = run_with(&cannot_open, source(300, 200), &[kyoto(2.0)]).await;

    let Err(InferenceError::Install(source)) = &installing else {
        panic!("a model that could not be put on disk returned {installing:?}");
    };
    assert!(source.to_string().contains("up_kyoto_2x_fp16"), "the failure did not name the model: {source}");
    assert!(
        matches!(opening, Err(InferenceError::Open { .. })),
        "a model that would not open landed on the same variant as one that was never transferred: {opening:?}"
    );

    // And neither produced an image or reached a tile, which is the half a caller sees.
    assert!(cannot_install.log().ran.is_empty(), "a model that was never installed still ran tiles");
}

#[tokio::test]
async fn a_run_whose_model_transfer_was_stopped_reports_a_cancellation_rather_than_a_failed_install() {
    // The distinction a user meets: stopping a download is something they did, and a run that reported it as an
    // install failure would tell them their enhancement broke. It is the same rule the tile loop already keeps
    // for a stop between tiles, applied to the transfer the stop now reaches.
    let backend = Fake { stops_installing: Some("up_kyoto_2x_fp16".to_string()), ..Fake::new() };

    let outcome = run_with(&backend, source(300, 200), &[kyoto(2.0)]).await;

    assert!(
        matches!(outcome, Err(InferenceError::Cancelled)),
        "a run whose transfer was stopped returned {outcome:?}"
    );
    assert!(backend.log().ran.is_empty(), "a run whose model never arrived still ran tiles");
}

#[tokio::test]
async fn a_model_that_cannot_be_opened_fails_the_run_naming_it() {
    let backend = Fake { fails_to_open: Some("up_kyoto_2x_fp16".to_string()), ..Fake::new() };

    let outcome = run_with(&backend, source(300, 200), &[kyoto(2.0)]).await;

    let Err(InferenceError::Open { artifact, .. }) = outcome else {
        panic!("a model that would not open returned {outcome:?}");
    };
    assert_eq!(artifact, "up_kyoto_2x_fp16");
    assert!(backend.log().ran.is_empty(), "a model that would not open still ran tiles");
}

#[tokio::test]
async fn a_chain_naming_one_model_twice_opens_it_once_and_never_reopens_it() {
    // The half of the guarantee that is about the chain's own conduct: it takes a handle per operation and holds
    // it, so nothing it does causes a model it is already holding to be opened a second time.
    let backend = Fake::new();

    run_with(&backend, source(300, 200), &[kyoto(2.0), kyoto(2.0)]).await.unwrap();

    assert_eq!(backend.log().acquired.len(), 2, "the chain did not ask for the model once per operation");
    assert_eq!(*backend.opens.lock().unwrap(), 1, "the chain reopened a model it was already holding");
    assert!(backend.freed.lock().unwrap().is_empty(), "a session was freed while the store still held it");
}

#[tokio::test]
async fn a_model_released_midway_stays_usable_to_the_chain_that_is_still_holding_it() {
    // The other half, and the one that needs something else to intervene: a release arriving between two of the
    // chain's operations drops the *store's* reference and nothing else, because the chain is holding its own.
    // The run therefore finishes against sessions that are still alive.
    //
    // The cache itself is where that ownership is checked — `sessions::tests::resident::
    // a_session_removed_while_in_use_stays_usable_and_the_next_request_gets_a_fresh_one` — and this is the chain
    // actually depending on it, which no test above could show because their backend opens a fresh session every
    // time and so has nothing to reclaim.
    let backend = Fake { release_before_acquire: Some(2), ..Fake::new() };

    let produced = run_with(&backend, source(300, 200), &[kyoto(2.0), kyoto(2.0)]).await.unwrap();

    // The second operation ran, and ran against a usable model: 2x and then 2x again over a 300x200 source is
    // only reachable by running both. Not `Log::passes`, which collapses consecutive repeats and so reads a
    // chain naming one model twice as a single pass.
    assert_eq!(produced.dimensions(), (1200, 800));
    assert_eq!(backend.log().acquired.len(), 2);

    // The release was real rather than a no-op the assertions above would pass without: the store had let go, so
    // the second operation was served a freshly opened session. That reopen is the releaser's doing, not the
    // chain's — which is what the test above pins by showing there is none without it.
    assert_eq!(*backend.opens.lock().unwrap(), 2, "the release never reached the store");

    // And on no tile of either operation had anything been freed under the run.
    assert!(
        backend.log().freed_at_tile.iter().all(|freed| *freed == 0),
        "a session was freed under a run still holding it: {:?}",
        backend.log().freed_at_tile
    );

    // The first session went when the chain returned and not before, which is the whole of "held until the run
    // finishes". The second is still in the store, so it is not in this list.
    assert_eq!(*backend.freed.lock().unwrap(), vec![1]);
}

#[tokio::test]
async fn cancelling_while_a_model_is_running_stops_the_run_and_returns_no_image() {
    // From *inside* a model rather than between operations: an upscale of a large photograph is the longest single
    // wait this application asks a user to sit through, and a cancel that took effect only at the end of it would
    // not be a cancel.
    let cancel = CancellationToken::new();
    let backend = Fake { cancels: Some(("up_kyoto_2x_fp16".to_string(), cancel.clone())), ..Fake::new() };

    let outcome = run_chain(
        &backend,
        source(600, 400),
        &[kyoto(2.0)],
        ExecutionProvider::Auto,
        ChannelDepth::Eight,
        None,
        &cancel,
        None,
    )
    .await;

    assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a cancelled run returned {outcome:?}");
    // It stopped between tiles rather than running the image out: 600x400 is six tiles at the default geometry.
    let ran = backend.log().ran.len();
    assert!(ran < 6, "the run covered {ran} tiles after being cancelled on the first");
}

#[tokio::test]
async fn a_run_cancelled_before_it_starts_opens_nothing() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let backend = Fake::new();

    let outcome = run_chain(
        &backend,
        source(300, 200),
        &[kyoto(2.0)],
        ExecutionProvider::Auto,
        ChannelDepth::Eight,
        None,
        &cancel,
        None,
    )
    .await;

    assert!(matches!(outcome, Err(InferenceError::Cancelled)));
    assert!(backend.log().acquired.is_empty(), "a cancelled run still installed a model");
}

#[tokio::test]
async fn every_operation_of_a_chain_produces_the_same_depth() {
    // The depth is resolved once, before anything runs, so it cannot depend on how many enhancements a user
    // happened to select.
    let backend = Fake::new();

    for chain in [vec![kyoto(2.0)], vec![kyoto(2.0), kyoto(2.0)], vec![kyoto(2.0), kyoto(2.0), kyoto(2.0)]] {
        let result = run_chain(
            &backend,
            source(300, 200),
            &chain,
            ExecutionProvider::Auto,
            ChannelDepth::Sixteen,
            None,
            &CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

        let result = result.0;
        assert!(matches!(*result, DynamicImage::ImageRgb16(_)), "a chain of {} produced {result:?}", chain.len());
    }
}

#[tokio::test]
async fn the_source_depth_is_resolved_against_the_image_that_was_loaded() {
    // Through `process` rather than `run`, because that is where the resolution now happens: `run` is handed an
    // answer, and the one caller that has the loaded image is the one that works it out.
    let backend = Fake::new();
    let sixteen = Picture::new(
        "/pictures/raw.dng",
        Arc::new(DynamicImage::ImageRgb16(ImageBuffer::new(300, 200))),
        "cafebabecafebabe",
    );
    let options = || Some(ProcessOptions { depth: OutputDepth::Source, ..Default::default() });

    let result = process(&backend, None, &sixteen, &[kyoto(2.0)], options()).await.unwrap().picture;

    assert!(
        matches!(result.pixels(), DynamicImage::ImageRgb16(_)),
        "a 16-bit source came back as {:?}",
        result.pixels()
    );

    // And the same request over an 8-bit source resolves the other way.
    let result = process(&backend, None, &picture(300, 200), &[kyoto(2.0)], options()).await.unwrap().picture;

    assert!(
        matches!(result.pixels(), DynamicImage::ImageRgb8(_)),
        "an 8-bit source came back as {:?}",
        result.pixels()
    );
}
