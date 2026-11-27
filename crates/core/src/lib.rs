// SPDX-License-Identifier: MIT
//! Orbe's core types: generational handles ([`Arena`]), resource paths ([`ResPath`]) and the
//! dynamic [`Variant`] value that scenes, resources and scripts exchange.
//!
//! Nothing here performs I/O. Everything that parses outside data returns a `Result`.

mod arena;
mod respath;
mod text;
mod variant;

pub use arena::{Arena, Handle};
pub use respath::{PathError, ResPath, Scheme};
pub use text::ParseError;
pub use variant::{Variant, MAX_DEPTH};
