//! Splitting a full-resolution CPU pass across cores, one band of rows per worker.
//!
//! Two callers from two families: Osaka's wavelet colour fix convolves its planes through it
//! (`models::upscale::osaka::colorfix`), and colorization's compose writes the photograph through it
//! (`models::colorization::compose`).

use std::sync::LazyLock;

/// Runs `row` over each of `dst`'s `height` rows of `row_len` elements, in parallel across the available cores.
///
/// `row` gets the row's index and only that row's slice of `dst`, so the output is bit-identical to a serial pass.
///
/// For non-ONNX CPU passes only: the runtime already threads its own graphs, and a second pool would compete with it.
/// A static split over `std::thread::scope` rather than a work-stealing pool, because the rows are uniform.
pub fn for_each_row<T, F>(dst: &mut [T], row_len: usize, height: usize, row: F)
where
    T: Send,
    F: Fn(usize, &mut [T]) + Sync,
{
    // Below this there is not enough work to pay for the threads; the tests here run planes of a few hundred pixels.
    const MIN_ROWS_PER_WORKER: usize = 64;

    // Read once for the process rather than per pass: the colour fix enters this sixty times for one image — two
    // axes, five levels, two planes, three channels — and the answer is a property of the machine, not of the row
    // being written.
    static AVAILABLE: LazyLock<usize> =
        LazyLock::new(|| std::thread::available_parallelism().map_or(1, std::num::NonZero::get));

    let workers = (*AVAILABLE).min(height.div_ceil(MIN_ROWS_PER_WORKER)).max(1);

    if workers == 1 {
        for (y, out) in dst.chunks_mut(row_len).enumerate().take(height) {
            row(y, out);
        }

        return;
    }

    let rows_per_worker = height.div_ceil(workers);
    let row = &row;

    std::thread::scope(|scope| {
        for (index, band) in dst.chunks_mut(rows_per_worker * row_len).enumerate() {
            let first = index * rows_per_worker;

            scope.spawn(move || {
                for (offset, out) in band.chunks_mut(row_len).enumerate() {
                    row(first + offset, out);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn a_row_longer_than_the_width_is_visited_once_with_its_own_slice() {
        // Three samples per pixel, which is the compose's row: a split that chunked by the width would hand each row a
        // third of its pixels, and one that miscounted the band would hand a row its neighbour's. Tall enough to split
        // on any machine with two cores or more, and the serial path answers the same on one that has fewer.
        let (width, height) = (5_usize, 300_usize);
        let row_len = 3 * width;

        let mut dst = vec![u16::MAX; row_len * height];
        let visits = Mutex::new(vec![0_u32; height]);

        for_each_row(&mut dst, row_len, height, |y, out| {
            assert_eq!(out.len(), row_len, "row {y} was handed a slice that is not one row long");
            visits.lock().unwrap()[y] += 1;

            for (index, sample) in out.iter_mut().enumerate() {
                *sample = u16::try_from(y * row_len + index).expect("the test's samples fit a u16");
            }
        });

        let visits = visits.into_inner().unwrap();
        assert!(visits.iter().all(|count| *count == 1), "some row was not visited exactly once: {visits:?}");

        // Every element written by the row it belongs to and by no other, which is what "its own slice" means.
        for (index, sample) in dst.iter().enumerate() {
            assert_eq!(usize::from(*sample), index, "element {index} was written by the wrong row");
        }
    }
}
