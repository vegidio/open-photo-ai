//! The one rule this crate takes a `std::sync::Mutex` by.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Takes `mutex`, treating a poisoned lock as usable.
///
/// The same rule as `opai`'s own crate-private helper, which is not reused for the reason [`crate::task`] gives.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // One rule for the whole crate, because every mutex in it guards the same kind of thing: a table of jobs, the
    // enhancement slot, a map the holder owns outright, or a progress thinner. A panic while holding one still leaves
    // it usable, and propagating the panic instead would break every future open, run or export of the session.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_is_taken_over_rather_than_panicked_on() {
        let mutex = Mutex::new(1);

        let _ = std::panic::catch_unwind(|| {
            let _held = mutex.lock().expect("the first lock is not poisoned");
            panic!("a holder panics");
        });
        assert!(mutex.is_poisoned(), "the test did not poison the lock it means to take over");

        *lock(&mutex) += 1;

        assert_eq!(*lock(&mutex), 2, "the value behind a poisoned lock was not usable");
    }
}
