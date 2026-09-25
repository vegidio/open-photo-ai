//! Carrying out a request: planning and running a chain of operations, running a data operation, and reporting on both.
//!
//! [`process`] plans a chain, refuses what it cannot serve before anything is installed, and runs it; [`execute`] runs
//! one operation whose result is not a picture. Both acquire their sessions through [`acquire`], compose their progress
//! through [`progress`], and resolve the depth a result is produced at through [`depth`].
//!
//! **This module names no model's code.** A request reaches its implementation through the dispatch seam `models`
//! owns ([`Operation::pipeline`](crate::models::Operation::pipeline)); what that implementation does is asked of the
//! contract in [`pipeline`](crate::pipeline), and the pixels it moves are `imaging`'s. The dependency runs one way:
//! `inference` depends on `models`, and nothing under `models` names `inference`.

pub(crate) mod acquire;
pub(crate) mod depth;
pub(crate) mod execute;
#[cfg(test)]
mod live;
pub(crate) mod process;
pub(crate) mod progress;
#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::convert::Infallible;

    use imaging::TilingError;

    use crate::task::Cancelled;

    #[test]
    fn a_cancelled_run_is_not_the_runtime_shutting_down() {
        // Two different facts with two different reports. A caller that could convert one into the other would tell
        // a user who cancelled their own export that the application failed.
        assert_ne!(TypeId::of::<TilingError<Infallible>>(), TypeId::of::<Cancelled>());
        assert_ne!(TilingError::<Infallible>::Cancelled.to_string(), Cancelled.to_string());

        assert_eq!(TilingError::<Infallible>::Cancelled.to_string(), "the run was cancelled before it finished");
    }
}
