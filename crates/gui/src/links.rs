//! The web pages this application links to, and how a user is sent to one.
//!
//! The About dialog's links and the navbar's "Update Available" open here rather than in the webview's own
//! JavaScript, for one reason: the window names *which* page it wants, never the address. [`Link`] is the whole of
//! what it can ask for, and the address each one opens is decided in this file.

// Its own module rather than a neighbour of `reveal.rs`: that module exists for one platform problem - the file
// manager has to be reached from a particular thread - which opening a URL does not share. `open_url` spawns a
// detached process and returns, so the command below is a plain synchronous one with no thread hand-off.
//
// Not the plugin's own command behind an `opener:allow-open-url` grant, which `capabilities/default.json` records the
// reasons against, and not an `open_url(url: String)` of this crate's own either: a command that opens whatever
// address the window sends is that permission again, without the scope that would have limited it.

use serde::{Deserialize, Serialize};

use crate::command::{CommandError, Ended, Traceparent, command_span, traced_sync};

/// A page the application links to.
///
/// Crosses the boundary as its camel-cased name - `"repository"`, `"website"`, `"releases"` - which
/// `frontend/ipc/links.ts` writes a second time. Any other string is refused while the command's arguments are read,
/// so it opens nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Link {
    /// The project's source repository.
    Repository,
    /// The author's website.
    Website,
    /// The project's releases, where "Update Available" sends the user.
    Releases,
}

// A macro rather than a `const` because `concat!` takes only literals, and the releases page is built on it at
// compile time so the two cannot name different projects.
/// The project's source repository.
macro_rules! repository {
    () => {
        "https://github.com/vegidio/open-photo-ai"
    };
}

impl Link {
    /// The address this link opens.
    ///
    /// Only the addresses live here. The labels drawn for them are in the frontend's `lib/constants.ts`, beside the
    /// copyright line, because they are what the dialog draws.
    pub(crate) const fn url(self) -> &'static str {
        match self {
            Self::Repository => repository!(),
            Self::Website => "https://vinicius.io",
            Self::Releases => concat!(repository!(), "/releases"),
        }
    }
}

/// Why a link could not be opened.
///
/// Tagged like [`crate::logs::LogsError`], so the frontend reads every command's rejection the same way.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum LinksError {
    // The plugin's error is not `Serialize`, so it crosses as its own sentence.
    /// The system would not open the page: in practice, no program is registered for `https`.
    #[error("{message}")]
    OpenLink {
        /// What went wrong, in full.
        message: String,
    },
}

impl CommandError for LinksError {
    fn ended(&self) -> Ended<'_> {
        match self {
            Self::OpenLink { .. } => Ended::Failed { error: self, recorded: false },
        }
    }
}

// The command's name is written once on the TypeScript side too, in `frontend/ipc/links.ts`; nothing in either
// toolchain notices when one of the two is renamed alone.
/// Open `link`'s page in the system's default browser.
///
/// # Errors
///
/// [`LinksError::OpenLink`] where the system would not open it.
#[tauri::command]
pub(crate) fn open_link(link: Link, traceparent: Traceparent) -> Result<(), LinksError> {
    open_with(link, traceparent, |url| tauri_plugin_opener::open_url(url, None::<&str>))
}

/// [`open_link`], with the system's opener passed in so a test can supply one that refuses.
fn open_with<E: std::fmt::Display>(
    link: Link,
    traceparent: Traceparent,
    open: impl FnOnce(&str) -> Result<(), E>,
) -> Result<(), LinksError> {
    traced_sync(command_span!("open_link", traceparent), || {
        open(link.url()).map_err(|error| open_failed(link, error))
    })
}

/// The error a failed open crosses the boundary as, naming the page that was asked for.
fn open_failed(link: Link, error: impl std::fmt::Display) -> LinksError {
    LinksError::OpenLink { message: format!("{} could not be opened in the browser: {error}", link.url()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_link_opens_its_own_page() {
        assert_eq!(Link::Repository.url(), "https://github.com/vegidio/open-photo-ai");
        assert_eq!(Link::Website.url(), "https://vinicius.io");
        assert_eq!(Link::Releases.url(), "https://github.com/vegidio/open-photo-ai/releases");
    }

    /// The Rust half of the names `frontend/ipc/links.test.ts` pins from the other side.
    #[test]
    fn the_window_names_a_link_by_the_strings_the_frontend_sends() {
        assert_eq!(serde_json::from_str::<Link>(r#""repository""#).expect("repository"), Link::Repository);
        assert_eq!(serde_json::from_str::<Link>(r#""website""#).expect("website"), Link::Website);
        assert_eq!(serde_json::from_str::<Link>(r#""releases""#).expect("releases"), Link::Releases);
    }

    /// The property the closed command exists for: nothing the window sends is an address.
    #[test]
    fn anything_else_the_window_sends_is_refused() {
        for sent in [
            r#""https://example.com""#,
            r#""Repository""#,
            r#""Releases""#,
            r#""github""#,
            r#""""#,
            r#"{"url":"https://example.com"}"#,
        ] {
            assert!(serde_json::from_str::<Link>(sent).is_err(), "{sent} was accepted as a link");
        }
    }

    #[test]
    fn the_error_crossing_ipc_carries_its_own_sentence() {
        let error = open_failed(Link::Website, "no application is registered for https");

        assert_eq!(
            serde_json::to_string(&error).expect("the error should serialize"),
            r#"{"kind":"openLink","message":"https://vinicius.io could not be opened in the browser: no application is registered for https"}"#
        );
    }

    #[test]
    fn a_link_that_would_not_open_is_recorded_by_the_wrapper() {
        assert_eq!(CommandError::ended(&open_failed(Link::Website, "refused")).cell(), "recorded by the wrapper");
    }

    #[test]
    fn a_link_the_system_would_not_open_is_recorded_once_naming_the_address() {
        let (recorded, answer) = crate::test_support::recorded(|| {
            open_with(Link::Releases, Traceparent::default(), |_| Err("no handler for https"))
        });
        assert!(answer.is_err());

        let [warning] = recorded.events.as_slice() else {
            panic!("expected exactly one record: {:#?}", recorded.events);
        };
        assert_eq!(warning.level, tracing::Level::WARN);
        assert_eq!(warning.field("command"), Some("open_link"));

        let error = warning.field("error").expect("the warning says why");
        assert!(error.contains("https://github.com/vegidio/open-photo-ai/releases"), "no address: {error}");
        assert!(error.contains("no handler for https"), "no reason: {error}");
    }
}
