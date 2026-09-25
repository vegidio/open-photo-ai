//! Image files: how they're admitted, what's recorded about them, and how their pixels reach the window.
//!
//! [`mod@files`] is what a file becomes and the set of them this application has admitted, with the commands
//! the window reaches both through. [`mod@serve`] is the scheme handler: what a request may ask for, where the
//! pixels come from, and what crosses back. [`mod@renditions`] is the cache the handler reads and fills.

// `pub(crate)` because `tauri::generate_handler!` in `src/lib.rs` expands a command's path into a macro the
// `#[tauri::command]` attribute generated beside the function, and a `use` re-export carries the function without
// it. So the commands are named through their own module there — `images::files::open_images` — and everything else
// through the re-exports below.
mod crop;
mod decoded;
pub(crate) mod files;
mod renditions;
mod serve;
#[cfg(test)]
mod test_support;

pub(crate) use crop::{Crop, load_framed};
// Reached from outside this module only by `faces`' tests, which pin the picture the detector is handed against
// the framing itself rather than against a transcription of how one is named.
#[cfg(test)]
pub(crate) use crop::cropped;
pub(crate) use files::Opened;
pub(crate) use renditions::Renditions;
pub(crate) use serve::{SCHEME, serve};
