//! The arithmetic over a model's timed runs.

use std::time::Duration;

/// What a model's timed runs looked like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// The fastest run.
    pub min: Duration,
    /// The slowest run.
    pub max: Duration,
    /// The arithmetic mean.
    pub mean: Duration,
    /// The middle run, or the mean of the two middle ones.
    pub median: Duration,
    /// The **sample** standard deviation, Bessel-corrected.
    pub std_dev: Duration,
}

/// Summarizes `durations`.
///
/// Accumulated as `f64` nanoseconds and rounded once at the end, so the mean does not inherit the truncation of an
/// integer division.
///
/// The deviation is the **sample** deviation (`n-1`): the runs are a sample of the runs the machine could have
/// produced, and at the default of five the population formula understates the spread by about 11%. With a single
/// run it is zero rather than undefined — a division by `n-1` of zero — which is the one case the correction has to
/// be written around rather than argued about.
///
/// An empty slice summarizes to all zeroes. It cannot arise from a sweep, where at least one timed run is required
/// by the flag's own validator, and returning a total rather than an `Option` keeps that check where it belongs.
pub fn compute(durations: &[Duration]) -> Stats {
    if durations.is_empty() {
        return Stats::default();
    }

    // A sorted copy: the caller's order is the run order, which is what `--verbose` prints and what would otherwise
    // be silently destroyed by summarizing it.
    let mut sorted: Vec<f64> = durations.iter().map(nanos).collect();
    sorted.sort_by(f64::total_cmp);

    let count = sorted.len();
    let mean = sorted.iter().sum::<f64>() / count as f64;

    let variance = if count > 1 {
        sorted.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / (count - 1) as f64
    } else {
        0.0
    };

    let middle = count / 2;
    let median = if count.is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    };

    Stats {
        min: duration(sorted[0]),
        max: duration(sorted[count - 1]),
        mean: duration(mean),
        median: duration(median),
        std_dev: duration(variance.sqrt()),
    }
}

/// Throughput, as `megapixels` of **input** over the median run.
///
/// Input, because it is the one thing every model has in common: a 4x upscaler's output is sixteen times its input
/// and a denoiser's is the same size, so a figure over the output would not compare two models at all. Median,
/// because it is not moved by a single scheduling hiccup.
///
/// A median of zero reports zero rather than an infinity — which is not a defensive flourish: a run served entirely
/// from the run cache can genuinely round to nothing, and `inf` in a table says less than `0.00` beside a row
/// whose provider report is empty.
pub fn megapixels_per_second(megapixels: f64, median: Duration) -> f64 {
    let seconds = median.as_secs_f64();

    if seconds <= 0.0 { 0.0 } else { megapixels / seconds }
}

/// One duration as `f64` nanoseconds.
fn nanos(duration: &Duration) -> f64 {
    duration.as_secs_f64() * 1e9
}

/// `nanos` back to a duration, rounded once.
fn duration(nanos: f64) -> Duration {
    Duration::from_nanos(nanos.round().max(0.0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Durations from whole milliseconds, which is what the hand-computed expectations below are written in.
    fn millis(values: &[u64]) -> Vec<Duration> {
        values.iter().copied().map(Duration::from_millis).collect()
    }

    #[test]
    fn an_odd_number_of_runs_takes_the_middle_one_as_the_median() {
        // 10, 20, 30, 40, 100 → mean 40, median 30.
        let stats = compute(&millis(&[30, 100, 10, 40, 20]));

        assert_eq!(stats.min, Duration::from_millis(10));
        assert_eq!(stats.max, Duration::from_millis(100));
        assert_eq!(stats.mean, Duration::from_millis(40));
        assert_eq!(stats.median, Duration::from_millis(30));

        // Sample variance: ((30² + 20² + 10² + 0² + 60²) in ms²) / 4 = (900+400+100+0+3600)/4 = 1250 ms², so the
        // deviation is √1250 ≈ 35.355339 ms. Hand-computed, and deliberately not by re-running the formula here.
        let expected = Duration::from_nanos(35_355_339);
        let difference = stats.std_dev.abs_diff(expected);
        assert!(difference < Duration::from_micros(1), "{:?} is not {expected:?}", stats.std_dev);
    }

    #[test]
    fn an_even_number_of_runs_takes_the_mean_of_the_two_middle_ones_as_the_median() {
        // 10, 20, 30, 40 → median (20 + 30) / 2 = 25, which is a value no run took.
        let stats = compute(&millis(&[40, 10, 30, 20]));

        assert_eq!(stats.median, Duration::from_millis(25));
        assert_eq!(stats.mean, Duration::from_millis(25));
        assert_eq!(stats.min, Duration::from_millis(10));
        assert_eq!(stats.max, Duration::from_millis(40));

        // Sample variance: (225 + 25 + 25 + 225) / 3 = 166.666… ms², deviation √166.666… ≈ 12.909944 ms.
        let expected = Duration::from_nanos(12_909_944);
        assert!(stats.std_dev.abs_diff(expected) < Duration::from_micros(1), "{:?}", stats.std_dev);
    }

    #[test]
    fn a_single_run_has_a_zero_deviation_and_four_figures_equal_to_it() {
        let stats = compute(&millis(&[371]));

        assert_eq!(stats.std_dev, Duration::ZERO);
        assert_eq!(stats.min, Duration::from_millis(371));
        assert_eq!(stats.max, Duration::from_millis(371));
        assert_eq!(stats.mean, Duration::from_millis(371));
        assert_eq!(stats.median, Duration::from_millis(371));
    }

    #[test]
    fn identical_runs_have_a_zero_deviation() {
        let stats = compute(&millis(&[50, 50, 50, 50, 50]));

        assert_eq!(stats.std_dev, Duration::ZERO);
        assert_eq!(stats.mean, Duration::from_millis(50));
        assert_eq!(stats.median, Duration::from_millis(50));
    }

    #[test]
    fn the_deviation_is_the_sample_one_rather_than_the_population_one() {
        let durations = millis(&[10, 20, 30, 40, 50]);
        let stats = compute(&durations);

        // Sample (n-1): variance 250 ms², deviation √250 ≈ 15.811388 ms.
        let sample = Duration::from_nanos(15_811_388);
        // Population (n): variance 200 ms², deviation √200 ≈ 14.142136 ms — about 11% lower, which is the whole
        // reason the correction is here rather than left to whichever formula was typed first.
        let population = Duration::from_nanos(14_142_136);

        assert!(stats.std_dev.abs_diff(sample) < Duration::from_micros(1), "{:?}", stats.std_dev);
        assert!(stats.std_dev > population, "{:?} is the population deviation", stats.std_dev);
    }

    #[test]
    fn the_caller_s_run_order_is_not_disturbed_by_summarizing_it() {
        // `--verbose` prints these in run order, so a thermal ramp the median hides stays visible.
        let durations = millis(&[30, 100, 10]);
        let before = durations.clone();

        let _ = compute(&durations);

        assert_eq!(durations, before);
    }

    #[test]
    fn the_mean_does_not_inherit_an_integer_divisions_truncation() {
        // Three runs of 1 ns, 1 ns and 2 ns: the true mean is 1.333… ns, which rounds to 1 ns — and a mean computed
        // over integers would also give 1. The value under test is that the rounding happens once, at the end: over
        // 1, 2 and 2 the true mean is 1.666…, which rounds to 2 rather than truncating to 1.
        let stats = compute(&[Duration::from_nanos(1), Duration::from_nanos(2), Duration::from_nanos(2)]);

        assert_eq!(stats.mean, Duration::from_nanos(2));
    }

    #[test]
    fn throughput_is_the_input_megapixels_over_the_median() {
        // 0.4096 MPix over a 200 ms median = 2.048 MPix/s.
        let rate = megapixels_per_second(0.4096, Duration::from_millis(200));

        assert!((rate - 2.048).abs() < 1e-9, "{rate}");
    }

    #[test]
    fn a_zero_median_reports_no_throughput_rather_than_dividing_by_zero() {
        let rate = megapixels_per_second(0.4096, Duration::ZERO);

        assert_eq!(rate, 0.0);
        assert!(rate.is_finite(), "a zero median produced {rate}");
    }

    #[test]
    fn no_runs_at_all_summarizes_to_zeroes_rather_than_panicking() {
        // Unreachable from a sweep, where `--runs` refuses zero. Asserted so that it stays unreachable by
        // construction rather than by luck.
        assert_eq!(compute(&[]), Stats::default());
    }
}
