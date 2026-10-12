## 1. The crate and its argument surface

- [x] 1.1 Create `crates/perf` producing the `perftest` binary — a workspace member beside `gui` and
      `cli`, depending on `opai`, `clap`, `comfy-table` (TTY feature off), `serde`, `serde_json`,
      `thiserror`, `tokio` and `rust-sak`'s `fs` feature, inheriting the workspace lints. Verify
      `cargo build -p perf` succeeds and `perftest --help` prints (D1, and the crate table under
      *Crates, dependencies and validation*).
- [x] 1.2 Declare the argument surface with `clap`'s derive API: `--runs`, `--warmup`, `--provider`,
      `--precision`, `--scale`, `--strength`, `--bias`, `--image`, `--cache`, `--skip-verify`,
      `--json`, `--verbose`, and the `list` subcommand. Verify by unit test that `--provider` parses
      through the library's own `FromStr for ExecutionProvider` and refuses a spelling it does not
      publish; that `--precision`'s three spellings map onto `Precision`'s values; that each
      parameter flag's bounds come from the catalogue's published `ParameterEntry` rather than from
      a literal; and that `--runs 0` is refused (D3).

## 2. Choosing what to measure

- [x] 2.1 Resolve the selection and render `list`, both over `catalogue()` and both before anything
      is initialized. Verify by unit test that no names selects every published variant at the
      requested precision, in catalogue order; that an unknown name fails naming it and pointing at
      `list`; that a name given twice is selected once; that a codename at a precision its variant
      does not publish is reported as the catalogue publishing no such row rather than as a model
      failure; that the parameter value is chosen by the published parameter's name so a family whose
      parameter has no flag fails loudly; and that `list` prints the catalogue unannotated, with no
      judgement about what can be run (D2, D3).

## 3. Starting up

- [x] 3.1 Install the log sink with `logging::init(APP_NAME, PERF)` and initialize against the shared
      configuration directory with `InitOptions { app: Some(PERF), models, on_progress }`, mapping
      `--skip-verify` to `ModelTrust::LocalFiles` and reporting install progress to stderr. Verify by
      running it that a first run reports the runtime download rather than looking hung, and that a
      run started while the GUI holds the claim exits with a message naming the holder and its pid
      rather than a generic failure (D11, D12).
- [x] 3.2 Load the input: the **workspace root's** `fixtures/test.dat` embedded with
      `include_bytes!` — shared with the live inference checks rather than copied under
      `crates/perf/`, which is what that directory exists for — materialized through `rust-sak`'s
      RAII temp handle and decoded by `opai::image::load`, or the photograph `--image` names instead.
      Verify by test that the embedded sample decodes to 640x640 and that a path that does not decode
      fails naming the file, and by running it that the header reports whichever was used with its
      dimensions and megapixels (D13).

## 4. The sweep

- [x] 4.1 Implement the per-model loop — release every resident session, the untimed warm-up runs,
      release again, the timed cold start, the timed runs — sequential across models, dropping the
      previous model's `Enhanced` before the next begins, each run given its own `Picture` identity
      over one shared pixel buffer, `ProcessOptions::cache` following `--cache` and `on_progress`
      left `None` throughout. Verify by a hand-run sweep that the cold start stands **consistently and
      measurably above** the median by the cost of building a session, where a release that did not
      take effect would leave the two within each other's run-to-run spread. How large that gap is, is
      the provider's business rather than a threshold to assert: on CoreML the compiled model is
      cached under `<config>/engines/`, so it is a couple of hundred milliseconds over a sub-second
      median rather than the multiple a cold graph optimization elsewhere would show. Also verify that
      a sweep with `--cache` performs an inference per timed run rather than being served from the
      store (D4, D5, D6).
- [x] 4.2 Compute the statistics over the timed runs — minimum, maximum, mean, median and
      Bessel-corrected sample standard deviation over `f64` nanoseconds, rounded once — and the
      throughput figure over input megapixels and the median. Verify by unit test against
      hand-computed values for an odd and an even number of runs, that a single run gives a zero
      deviation with the other four equal to it, and that the throughput of a zero median is not a
      division by zero (D14).
- [x] 4.3 Model the four outcomes — completed, declined, failed, interrupted — telling a decline from
      a failure by the library's own `InferenceError::Unsupported` rather than by anything this crate
      decides; fold the provider report across each model's runs; and map the outcomes to the exit
      status. Verify by unit test that a declined model is dropped from an unnamed sweep and reported
      with its reason when named; that a union containing anything but the requested provider is
      reported as a downgrade; that an empty set is reported as nothing having executed rather than as
      the requested provider; that `Auto` reports what it resolved to instead of a downgrade; and that
      only failures and an interruption produce a non-zero status while declines alone exit zero (D2,
      D7, and the exit-status requirement).
- [x] 4.4 Cancel the run in flight on Ctrl-C through a `CancellationToken` passed as
      `ProcessOptions::cancel`, mark that model interrupted rather than failed, break out of the
      sweep and still print the summary. Verify by interrupting a real sweep that the summary for the
      finished models is printed, the interrupted model is marked as such, and the exit status is 1 —
      and by grep that nothing in the sweep calls `std::process::exit` (D8's listener seam, and the
      interruption requirement).

## 5. Reporting

- [x] 5.1 Render the plain report: the header — image, requested provider beside `Opai::providers()`,
      precision, run counts with the warning where the warm-up is zero, parameters, cache state and
      `cache_mode()`, log path, models selected — then the summary table through `comfy-table`, the
      failures, the declines with their reasons, the count of those skipped, the downgrade warnings
      and the footer. Verify by
      unit test that the header states every one of those facts and that a duration renders at a
      fixed width in both the sub-second and the multi-second form, and by running it that a table
      pasted into an issue carries the conditions that produced it (D8, D9). `--verbose` additionally
      prints every timed run in run order.
- [x] 5.2 Render the JSON document under `--json` — a schema version, the tool's version, the
      configuration, and per-model outcomes carrying durations in milliseconds as numbers and the
      reason for each decline, failure and interruption. Verify by unit test that the document round
      trips through `serde_json` and that each of the four outcomes is distinguishable in it, and by
      running `perftest --json > results.json` on a machine that downloads a runtime on the way that
      the file parses — which is the check that progress and diagnostics went to stderr and the
      report alone to stdout (D6, D8).

## 6. Documentation and checks

- [x] 6.1 Write `crates/perf/README.md` — what each stage measures and why the cold start follows the
      warm-up, the flags, how to read the output, and the three things that will otherwise surprise
      someone: a sweep is refused while the GUI is running, `--cache` writes into the cache the GUI
      shares, and a sweep given no names skips the models the library does not yet run. Verify every
      command it shows runs as written.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace and verify all
      three are clean.
