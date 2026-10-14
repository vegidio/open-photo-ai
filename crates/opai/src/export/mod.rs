//! What an exported photograph is called, where it goes, which format it is written in, and how its name is claimed.
//!
//! Three parts, each a question every front end asks of the same rule:
//!
//! - [`mod@format`]: the eight formats an export can be written in, spelled as the front ends spell them
//!   ([`ExportFormat`]), a [`FormatChoice`] that is one of them or Preserve, and what each can do
//!   ([`capabilities`]).
//! - [`mod@name`]: the name and the destination a source is exported under ([`name_for`], [`destination_for`]): the
//!   prefix, the source's name, the suffix and the extension the format choice decides, inside the chosen folder or
//!   beside the source. Pure: it touches no file and cannot fail, so it is cheap enough to ask on every keystroke.
//! - [`mod@claim`]: holding that destination while the file is written ([`claim()`]) — the first free numbered name
//!   without overwriting, or a hidden name renamed over the destination on [`Claim::commit`] with it — and giving it
//!   back when the write does not finish.
//! - [`mod@write`]: the claim, the save and the commit together ([`write()`]), which is how a front end writes one.
//!
//! # Why this sits beside `image` and not inside it
//!
//! [`image`](crate::image) reads and writes one file at a path its caller names. Choosing that path — the naming
//! convention, numbering, overwriting through a hidden file — is an export policy built on top of it, and the claim is
//! not image I/O at all.

pub mod claim;
pub mod format;
pub mod name;
pub mod write;

pub use claim::{Claim, claim};
pub use format::{
    ExportFormat, ExportFormats, FormatCapability, FormatChoice, QualityRange, UnknownFormat, capabilities,
};
pub use name::{Named, Naming, destination_for, name_for};
pub use write::{WriteError, WriteOptions, Written, write};
