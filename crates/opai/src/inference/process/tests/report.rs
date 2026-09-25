//! What a run reports it was executed on.

use super::*;

/// One report's verdict, by the two fields it is read from.
fn verdict_of(requested: ExecutionProvider, actual: Vec<ExecutionProvider>) -> ProviderVerdict {
    ProviderReport { requested, actual }.verdict()
}

#[test]
fn a_run_that_got_what_it_asked_for_is_not_reported_as_a_downgrade() {
    let as_requested = verdict_of(ExecutionProvider::CoreMl, vec![ExecutionProvider::CoreMl]);

    assert_eq!(as_requested, ProviderVerdict::AsRequested(ExecutionProvider::CoreMl));
}

#[test]
fn a_union_containing_anything_but_the_requested_provider_is_a_downgrade() {
    // The silent case this exists for: a model that could not be opened on the GPU completes on the CPU,
    // returns the same image in the same way, and is several times slower.
    let downgraded = verdict_of(ExecutionProvider::Cuda, vec![ExecutionProvider::Cpu]);

    assert_eq!(
        downgraded,
        ProviderVerdict::Downgraded { requested: ExecutionProvider::Cuda, actual: vec![ExecutionProvider::Cpu] }
    );

    // And a chain that got the GPU for one session and not another is still a downgrade: the row is not a
    // timing of what was asked for.
    assert!(matches!(
        verdict_of(ExecutionProvider::Cuda, vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu]),
        ProviderVerdict::Downgraded { .. }
    ));
}

#[test]
fn an_empty_set_is_nothing_having_executed_rather_than_the_requested_provider() {
    // `report.actual.first().unwrap_or(report.requested)` is the line this exists to stop being written.
    let nothing = verdict_of(ExecutionProvider::Cuda, Vec::new());

    // Not a downgrade either: nothing ran, so there is no provider to compare against the request.
    assert_eq!(nothing, ProviderVerdict::NothingExecuted);
}

#[test]
fn auto_reports_what_it_resolved_to_instead_of_a_downgrade() {
    // "Downgraded" is not a meaningful judgement against a request that asked for whatever the machine has.
    let resolved = verdict_of(ExecutionProvider::Auto, vec![ExecutionProvider::CoreMl]);

    assert_eq!(resolved, ProviderVerdict::Resolved(vec![ExecutionProvider::CoreMl]));

    // Including when it resolved to the CPU, which is an ordinary answer on a machine with no GPU provider.
    assert_eq!(
        verdict_of(ExecutionProvider::Auto, vec![ExecutionProvider::Cpu]),
        ProviderVerdict::Resolved(vec![ExecutionProvider::Cpu])
    );
}

#[tokio::test]
async fn a_run_every_model_of_which_opened_on_the_provider_asked_for_reports_only_it() {
    let mut backend = Fake::new();
    backend.builds_on = ExecutionProvider::CoreMl;

    let (_, report) = run_reporting(&backend, source(120, 80), &[kyoto(2.0)], ExecutionProvider::CoreMl)
        .await
        .unwrap();

    assert_eq!(report.requested, ExecutionProvider::CoreMl);
    assert_eq!(
        report.actual,
        vec![ExecutionProvider::CoreMl],
        "the honoured request was reported as something else"
    );
}

#[tokio::test]
async fn a_run_the_requested_provider_could_not_serve_reports_the_downgrade_on_the_result() {
    // The fact this whole report exists for. The run succeeds and produces the same image it would have on the
    // GPU, several times slower, so the result is the only thing that can say the machine did not serve what was
    // asked for — and nothing here registered a progress handler, which is the caller that would otherwise lose
    // it entirely.
    let mut backend = Fake::new();
    backend.builds_on = ExecutionProvider::Cpu;

    let (pixels, report) =
        run_reporting(&backend, source(120, 80), &[kyoto(2.0)], ExecutionProvider::Cuda).await.unwrap();

    assert_eq!(pixels.dimensions(), (240, 160), "the downgraded run did not produce the enhanced image");
    assert_eq!(report.requested, ExecutionProvider::Cuda);
    assert_eq!(report.actual, vec![ExecutionProvider::Cpu]);
    assert_ne!(report.requested, report.actual[0], "the downgrade is not legible from the result");
}

#[tokio::test]
async fn a_chain_downgraded_for_its_second_model_alone_reports_both_providers() {
    // Whether a provider can open a model is decided per model, so this run genuinely happened on two of them —
    // and a report naming one would describe a run that did not. It is also the case the reference cannot report:
    // its handler fires once, for the first downgrade, and its latch keeps every later model quiet.
    let mut backend = Fake::new();
    backend.builds_on = ExecutionProvider::Cuda;
    backend.built_on.insert("up_kyoto_2x_fp16".to_string(), ExecutionProvider::Cpu);

    // 4x, then 2x: two operations, one artifact each, and only the second falls back.
    let chain = [kyoto(4.0), kyoto(2.0)];
    let (_, report) = run_reporting(&backend, source(120, 80), &chain, ExecutionProvider::Cuda).await.unwrap();

    assert_eq!(report.requested, ExecutionProvider::Cuda);
    assert_eq!(report.actual, vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu]);
}

#[tokio::test]
async fn a_chain_taking_four_handles_on_one_provider_names_it_once() {
    // Two 8x operations are two passes each, so four acquisitions — and nothing a caller does with "what did this
    // run on" is served by being told the same answer four times. The order is what makes the report comparable
    // between two runs of the same shape rather than merely equal as a set.
    let mut backend = Fake::new();
    backend.builds_on = ExecutionProvider::CoreMl;

    let chain = [kyoto(8.0), kyoto(8.0)];
    let (_, report) = run_reporting(&backend, source(60, 40), &chain, ExecutionProvider::Auto).await.unwrap();

    assert_eq!(backend.log().acquired.len(), 4, "this chain did not take four handles");
    assert_eq!(report.requested, ExecutionProvider::Auto);
    assert_eq!(report.actual, vec![ExecutionProvider::CoreMl], "one provider was reported once per session");
}

#[tokio::test]
async fn an_empty_chain_reports_what_was_asked_for_and_nothing_executed() {
    // The third outcome, and the one a caller gets wrong by writing
    // `report.actual.first().unwrap_or(report.requested)`: no session was built, so claiming the run happened on
    // CUDA would tell a benchmark it measured something it never took.
    let backend = Fake::new();

    let enhanced = process(
        &backend,
        None,
        &picture(300, 200),
        &[],
        Some(ProcessOptions { provider: ExecutionProvider::Cuda, ..Default::default() }),
    )
    .await
    .unwrap();

    assert_eq!(enhanced.providers.requested, ExecutionProvider::Cuda);
    assert!(enhanced.providers.actual.is_empty(), "an empty chain claimed to have executed on something");
}

#[tokio::test]
async fn a_run_served_entirely_from_the_store_reports_what_was_asked_for_and_nothing_executed() {
    // The same third outcome reached the other way. This run had operations and produced the enhanced image; it
    // simply acquired no session, because every step was already stored — so what it can honestly say about
    // execution providers is nothing.
    let backend = Fake::new();
    let cache = store();
    let source = picture(120, 80);
    let chain = [kyoto(2.0)];
    let on = |provider| Some(ProcessOptions { provider, ..Default::default() });

    // Fills the store.
    process(&backend, Some(&cache), &source, &chain, on(ExecutionProvider::Cpu)).await.unwrap();
    let acquired = backend.log().acquired.len();

    let served = process(&backend, Some(&cache), &source, &chain, on(ExecutionProvider::CoreMl)).await.unwrap();

    assert_eq!(backend.log().acquired.len(), acquired, "the served run acquired a session after all");
    assert_eq!(served.providers.requested, ExecutionProvider::CoreMl);
    assert!(served.providers.actual.is_empty(), "a fully-served run claimed to have executed on something");
}

#[tokio::test]
async fn the_result_identity_is_the_same_on_two_providers_that_both_actually_ran() {
    // image-enhancement's final scenario, with the cache **off** on both runs so that the equality is a property
    // of how the identity is composed rather than an artefact of the second run being served the first one's
    // entry. Nothing keyed on the identity moves because a machine's GPU was busy.
    let mut gpu = Fake::new();
    gpu.builds_on = ExecutionProvider::CoreMl;
    let cpu = Fake::new();

    let source = picture(120, 80);
    let chain = [kyoto(2.0)];
    let off = |provider| Some(ProcessOptions { provider, cache: false, ..Default::default() });

    let on_gpu = process(&gpu, None, &source, &chain, off(ExecutionProvider::CoreMl)).await.unwrap();
    let on_cpu = process(&cpu, None, &source, &chain, off(ExecutionProvider::Cpu)).await.unwrap();

    // Both actually ran, and on different providers — which is what makes the equality below mean anything.
    assert_eq!(on_gpu.providers.actual, vec![ExecutionProvider::CoreMl]);
    assert_eq!(on_cpu.providers.actual, vec![ExecutionProvider::Cpu]);

    assert_eq!(
        on_gpu.picture.identity(),
        on_cpu.picture.identity(),
        "the provider reached the identity, which would halve the cache's hit rate on a mixed machine"
    );
}

#[test]
fn the_report_renders_both_halves_when_it_is_printed() {
    // A front end that logs one of these, and every test above that prints one on failure, reads the requested
    // provider beside what ran — a `Debug` that showed one would make the downgrade invisible again.
    let report = ProviderReport::new(ExecutionProvider::Cuda, [ExecutionProvider::Cpu]);
    let rendered = format!("{report:?}");

    assert!(rendered.contains("Cuda"), "the requested provider is not rendered: {rendered}");
    assert!(rendered.contains("Cpu"), "what ran is not rendered: {rendered}");
}
