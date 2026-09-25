//! Who a process says it is, and how it is named to a user.

use std::fmt;

use crate::error::InitError;

/// The identity the Tauri desktop application declares.
pub const GUI: &str = "gui";

/// The identity the terminal application declares.
pub const CLI: &str = "cli";

/// The identity the benchmark harness declares.
pub const PERF: &str = "perf";

/// Rejects an identity that would not survive being recorded.
///
/// Returns the rule that was broken, which each entry point turns into its own error: the identity reaches the
/// library at two of them — [`Opai::initialize`](crate::Opai::initialize) and
/// [`logging::init`](crate::logging::init) — and they report through different types.
pub(crate) fn check(app: &str) -> Result<(), &'static str> {
    // The rule is read off the record format rather than chosen for tidiness. `instance` writes `<app> <pid>` on one
    // line and reads it back by splitting on the first space, so an identity carrying a space comes back as a
    // different identity beside an unparseable process id — which `instance::read_holder` answers with `None`, a
    // refusal that names no holder. That is safe and it is silent, and the symptom is a refused user being told
    // something is running without being told what: the one thing the record exists to prevent. An empty identity is
    // worse in its own way, rendering `(, pid 4821)`. A control character breaks the line itself, and in the log the
    // one-record-per-line property the formatter rests on.
    //
    // Refusing here rather than coping in `read_holder` is deliberate. That reader answers `None` to everything it
    // cannot make sense of and that is never a grant; carrying a space through it would mean an escape or a quoting
    // scheme, which is the reader deciding more rather than less.
    //
    // Sanitising — replacing the space, truncating — was rejected for the same reason it usually is: two distinct
    // applications would quietly record one identity, which is the collision this exists to avoid.
    //
    // The rule is here once, so the two entry points cannot come to disagree about what is recordable.
    match app {
        "" => Err("it is empty"),
        // Whitespace before control characters, so a tab is reported as the thing a reader of it will recognise.
        _ if app.chars().any(char::is_whitespace) => Err("it contains whitespace"),
        _ if app.chars().any(char::is_control) => Err("it contains a control character"),
        _ => Ok(()),
    }
}

/// As [`check`], reported as the error [`Opai::initialize`](crate::Opai::initialize) returns.
pub(crate) fn validate(app: &str) -> Result<(), InitError> {
    check(app).map_err(|reason| InitError::InvalidApp { app: app.to_string(), reason })
}

/// The identity a process initializing under `name` is known by, `declared` where it declared one.
///
/// An application that declares none is known by the name it initialized under.
///
/// # Errors
///
/// Returns [`InitError::InvalidApp`] where the resolved identity would not survive being recorded — see [`validate`].
/// That applies to the defaulted identity as much as to a declared one: `name` is validated as a directory segment,
/// which permits a space, and a space is the one thing the record cannot carry.
pub(crate) fn identity(declared: Option<String>, name: &str) -> Result<String, InitError> {
    // The name rather than a fixed default, because every alternative invents something: a fixed `cli` labels a desktop
    // application as a terminal one on every line of its log and tells a refused user to close something that is not
    // running, and a placeholder like `app` makes two embedders on one machine indistinguishable in a file whose whole
    // purpose is telling interleaved sessions apart. The name is already in hand, is what every directory of that
    // installation is resolved from, and is never a guess.
    let app = declared.unwrap_or_else(|| name.to_string());
    validate(&app)?;
    Ok(app)
}

/// The process holding the single-instance claim, as a refusal names it.
///
/// What is written into the holder record beside the lock file and read back out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    // Both fields together, because neither is useful alone: a process id with no application does not say what to
    // close, and an application with no process id does not say *which* copy of it.
    /// The identity the holding process declared, or the name it initialized under where it declared none.
    pub app: String,
    /// Its process id, as the operating system reports it.
    pub pid: u32,
}

impl fmt::Display for Holder {
    /// `gui, pid 4821` — the inside of the parenthesis a refusal puts it in.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, pid {}", self.app, self.pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identities this project's own binaries declare.
    ///
    /// Pinned here as the one list the tests below walk, so a fourth binary is covered by adding its constant in one
    /// place.
    const OURS: [&str; 3] = [GUI, CLI, PERF];

    /// Unwraps the identity an [`InitError::InvalidApp`] rejected and the reason it gave, failing on any other
    /// variant.
    fn rejected(error: InitError) -> (String, &'static str) {
        match error {
            InitError::InvalidApp { app, reason } => (app, reason),
            other => panic!("expected InvalidApp, got {other:?}"),
        }
    }

    #[test]
    fn the_spellings_are_the_published_ones_and_are_distinct() {
        // Pinned as literals, because they are what a refusal prints and what a log record carries: a rename is a
        // visible change and should have to be made here too.
        assert_eq!(OURS, ["gui", "cli", "perf"]);
    }

    #[test]
    fn an_identity_that_would_not_survive_being_recorded_is_refused() {
        // A space is the one that motivates the rule — `<app> <pid>` split on the first space — and the rest are the
        // ways a line stops being one line or stops naming anything.
        for app in ["my app", " gui", "gui "] {
            assert_eq!(rejected(validate(app).unwrap_err()), (app.to_string(), "it contains whitespace"));
        }

        for app in ["gui\tcli", "gui\nperf", "\r"] {
            assert_eq!(rejected(validate(app).unwrap_err()), (app.to_string(), "it contains whitespace"));
        }

        assert_eq!(
            rejected(validate("gui\u{7}").unwrap_err()),
            ("gui\u{7}".to_string(), "it contains a control character")
        );
        assert_eq!(
            rejected(validate("gui\0").unwrap_err()),
            ("gui\0".to_string(), "it contains a control character")
        );
        assert_eq!(rejected(validate("").unwrap_err()), (String::new(), "it is empty"));
    }

    #[test]
    fn an_identity_that_records_legibly_is_accepted() {
        // This project's three, and the shape an embedder's own name takes — `APP_NAME` itself, which is what an
        // embedder that declares nothing is known by.
        for app in [GUI, CLI, PERF, "io.vinicius.opai", "com.example.Photos", "opai-test"] {
            validate(app).unwrap_or_else(|error| panic!("{app:?} was refused: {error}"));
        }
    }

    #[test]
    fn an_undeclared_identity_is_the_name_the_caller_initialized_under() {
        assert_eq!(identity(None, "io.vinicius.opai").unwrap(), "io.vinicius.opai");
    }

    #[test]
    fn a_declared_identity_wins_over_the_name() {
        assert_eq!(identity(Some(GUI.to_string()), "io.vinicius.opai").unwrap(), "gui");
    }

    #[test]
    fn a_name_that_cannot_be_recorded_is_refused_as_the_default_identity() {
        // `config::validate_name` accepts a space, because a directory may carry one. The record cannot, and the
        // default identity is subject to the same rule as a declared one rather than exempt from it.
        assert_eq!(
            rejected(identity(None, "my app").unwrap_err()),
            ("my app".to_string(), "it contains whitespace")
        );
    }

    #[test]
    fn a_holder_renders_as_the_application_and_its_process_id() {
        assert_eq!(Holder { app: GUI.to_string(), pid: 4821 }.to_string(), "gui, pid 4821");
        assert_eq!(Holder { app: PERF.to_string(), pid: 1 }.to_string(), "perf, pid 1");
        assert_eq!(
            Holder { app: "com.example.Photos".to_string(), pid: 9 }.to_string(),
            "com.example.Photos, pid 9"
        );
    }
}
