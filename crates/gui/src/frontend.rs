//! The window's own failures, written into the log.
//!
//! What the window runs never passes through `opai` or through [`crate::command`], so a render that throws, a
//! rejection nobody handled or a failure the window catches and carries on through would otherwise reach only the
//! webview's console. [`log`] is how the window says what happened. What becomes of it is decided here: the record's
//! target, its level and how long each of its texts may be.
//!
//! **The records are the file's alone.** Each is marked [`opai::logging::FILE_ONLY`], so the collector never receives
//! it. The window sends the same failure to Grafana itself, through Faro, where it arrives as an exception with its
//! session and view; sending the record too would put every window failure in Grafana twice.
//!
//! Which failures are worth a record is the window's decision, in `frontend/lib/report.ts`: a command's rejection is
//! already in the file, and only the window can tell one from its own failure.

// Not in `logs.rs`: that module is where the log file is and how a user is shown it. A record written from the window
// is a different subject, and its target should read as the window's.

use serde::Deserialize;

/// The target every record from the window is written under.
pub(crate) const TARGET: &str = "gui::frontend";

// Two only. A render crash is what the window does not survive, and every other failure it observes is a warning. It
// has nothing informational to say yet; when it does, a third variant is one line here and one in `ipc/log.ts`.
/// How severe the window judged its failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Level {
    Warn,
    Error,
}

/// One record, as the window describes it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowRecord {
    level: Level,
    /// What the window was doing, in its own sentence.
    message: String,
    /// The error's own text.
    error: Option<String>,
    /// Where it was thrown, as the webview formats a stack.
    stack: Option<String>,
    /// For a render crash, the path of components it was thrown under.
    component_stack: Option<String>,
}

// A bound on size, not a defence: the window is this application's own code. The largest record is about 21 KiB, so
// the fifty the window sends per load at most come to about a megabyte.
/// The longest `message` a record keeps, in bytes.
const MESSAGE_CAP: usize = 1024;
/// The longest `error` a record keeps, in bytes.
const ERROR_CAP: usize = 4 * 1024;
/// The longest `stack` or `component_stack` a record keeps, in bytes.
const STACK_CAP: usize = 8 * 1024;

/// What a text cut short ends with.
const CUT: &str = "… (cut short)";

// The command's name is written once more, in `frontend/ipc/log.ts`.
/// Write one record from the window to the file, under [`TARGET`] and at its level.
///
/// Marked [`opai::logging::FILE_ONLY`]: the collector never receives it, because the window reports the same failure
/// to Grafana through Faro.
///
/// **Not traced.** Every other command but `set_analytics` runs inside a span named after it, and a span here would
/// root one trace per failure holding nothing but the record, which is the whole of what matters.
///
/// Infallible: a malformed argument is refused by Tauri before this runs, and the window drops that refusal.
#[tauri::command]
pub(crate) fn log(record: WindowRecord) {
    let message = capped(&record.message, MESSAGE_CAP);
    let error = record.error.as_deref().map(|text| capped(text, ERROR_CAP));
    let stack = record.stack.as_deref().map(|text| capped(text, STACK_CAP));
    let component_stack = record.component_stack.as_deref().map(|text| capped(text, STACK_CAP));

    // `tracing`'s macros take the level at compile time, hence one arm per level. A field left `None` is not written.
    // The fields sit in a block of their own because a constant's name, the marker, cannot lead a level macro's list.
    match record.level {
        Level::Warn => tracing::warn!(
            target: TARGET,
            {
                error = error.as_deref(),
                stack = stack.as_deref(),
                component_stack = component_stack.as_deref(),
                { opai::logging::FILE_ONLY } = true,
            },
            "{message}"
        ),
        Level::Error => tracing::error!(
            target: TARGET,
            {
                error = error.as_deref(),
                stack = stack.as_deref(),
                component_stack = component_stack.as_deref(),
                { opai::logging::FILE_ONLY } = true,
            },
            "{message}"
        ),
    }
}

/// `text`, or as much of it as fits in `cap` bytes followed by [`CUT`]. Never splits a character.
fn capped(text: &str, cap: usize) -> std::borrow::Cow<'_, str> {
    if text.len() <= cap {
        return text.into();
    }

    format!("{}{CUT}", &text[..text.floor_char_boundary(cap)]).into()
}

#[cfg(test)]
mod tests {
    use tracing::Level as TracingLevel;

    use super::*;
    use crate::test_support::recorded;

    fn record(level: Level, message: &str) -> WindowRecord {
        WindowRecord { level, message: message.to_string(), error: None, stack: None, component_stack: None }
    }

    #[test]
    fn each_level_lands_at_its_own_under_the_windows_target() {
        for (level, expected) in [(Level::Warn, TracingLevel::WARN), (Level::Error, TracingLevel::ERROR)] {
            let (recorded, ()) = recorded(|| log(record(level, "the preview could not be drawn")));

            let [event] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
            assert_eq!(event.level, expected);
            assert_eq!(event.target, TARGET);
            assert_eq!(event.field("message"), Some("the preview could not be drawn"));
            // The field the collector's filter drops a record for, so the window's failures reach Grafana through
            // Faro alone. The filter itself is `opai`'s, and tested there.
            assert_eq!(event.field(opai::logging::FILE_ONLY), Some("true"), "{level:?} was not marked file-only");
        }
    }

    #[test]
    fn the_texts_are_written_when_given_and_absent_otherwise() {
        let (bare, ()) = recorded(|| log(record(Level::Warn, "bare")));
        let [event] = bare.events.as_slice() else { panic!("{:#?}", bare.events) };
        for field in ["error", "stack", "component_stack"] {
            assert_eq!(event.field(field), None, "{field} was written though not given");
        }

        let (recorded, ()) = recorded(|| {
            log(WindowRecord {
                error: Some("TypeError: x is undefined".to_string()),
                stack: Some("draw@app.js:1:2\nrender@app.js:3:4".to_string()),
                component_stack: Some("\n    at Canvas\n    at App".to_string()),
                ..record(Level::Error, "full")
            })
        });
        let [event] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
        assert_eq!(event.field("error"), Some("TypeError: x is undefined"));
        assert_eq!(event.field("stack"), Some("draw@app.js:1:2\nrender@app.js:3:4"));
        assert_eq!(event.field("component_stack"), Some("\n    at Canvas\n    at App"));
    }

    #[test]
    fn each_cap_holds_at_its_length_and_cuts_one_past_it() {
        let written = |record: WindowRecord| {
            let (recorded, ()) = recorded(|| log(record));
            let [event] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
            event.fields.clone()
        };
        let get = |fields: &[(String, String)], key: &str| {
            fields.iter().find(|(name, _)| name == key).map(|(_, value)| value.clone()).expect("written")
        };

        for (field, cap) in [
            ("message", MESSAGE_CAP),
            ("error", ERROR_CAP),
            ("stack", STACK_CAP),
            ("component_stack", STACK_CAP),
        ] {
            let with = |text: String| {
                let mut record = record(Level::Warn, "m");
                match field {
                    "message" => record.message = text,
                    "error" => record.error = Some(text),
                    "stack" => record.stack = Some(text),
                    _ => record.component_stack = Some(text),
                }
                record
            };

            let at = "a".repeat(cap);
            assert_eq!(get(&written(with(at.clone())), field), at, "{field} was cut at its cap");

            let past = get(&written(with("a".repeat(cap + 1))), field);
            assert_eq!(past, format!("{}{CUT}", "a".repeat(cap)), "{field} was not cut one past its cap");
        }
    }

    #[test]
    fn a_character_straddling_the_cap_is_not_split() {
        // Three bytes, starting one before the cap: keeping it whole would overrun, so it goes.
        let text = format!("{}€", "a".repeat(MESSAGE_CAP - 1));

        assert_eq!(capped(&text, MESSAGE_CAP), format!("{}{CUT}", "a".repeat(MESSAGE_CAP - 1)));
    }

    #[test]
    fn no_span_is_opened() {
        let (recorded, ()) = recorded(|| log(record(Level::Error, "crash")));

        assert!(recorded.spans.is_empty(), "the record opened a span: {:#?}", recorded.spans);
    }

    #[test]
    fn the_windows_json_shape_deserializes_and_an_unknown_level_is_refused() {
        let record: WindowRecord = serde_json::from_value(serde_json::json!({
            "level": "error",
            "message": "m",
            "error": "e",
            "stack": "s",
            "componentStack": "c",
        }))
        .expect("the window's shape");
        assert_eq!(record.level, Level::Error);
        assert_eq!(record.component_stack.as_deref(), Some("c"));

        let bare: WindowRecord =
            serde_json::from_value(serde_json::json!({ "level": "warn", "message": "m" })).expect("optional texts");
        assert_eq!((bare.level, bare.error), (Level::Warn, None));

        let refused = serde_json::from_value::<WindowRecord>(serde_json::json!({ "level": "info", "message": "m" }));
        assert!(refused.is_err(), "an unknown level was accepted");
    }
}
