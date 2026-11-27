// SPDX-License-Identifier: MIT
//! Orbe's scripting language.
//!
//! A small, sandboxed language for game logic. It has no access to files, the network or the
//! process: a script can only compute and call the functions the host hands it through [`Host`].
//! Every resource is bounded (source size, nesting, call depth, steps, string and array length),
//! so a hostile or buggy script ends with a [`ScriptError`], never a hang or a crash.
//!
//! ```text
//! fn fib(n) {
//!     if n < 2 { return n }
//!     return fib(n - 1) + fib(n - 2)
//! }
//! let total = 0
//! for i in range(10) { total += fib(i) }
//! ```
//!
//! Statements end at a newline or `;`. Values are [`orbe_core::Variant`]s; arrays and
//! dictionaries are values, so `a[i] = v` changes the variable `a` and nothing else.

mod interp;
mod lexer;
mod parser;

pub use interp::{Host, Limits, NoHost, Vm};
pub use parser::Script;

use std::fmt;

/// A parse or run-time failure, with the line it happened on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    /// What went wrong.
    pub message: String,
    /// 1-based line, or 0 when unknown.
    pub line: u32,
}

impl ScriptError {
    pub(crate) fn new(line: u32, message: impl Into<String>) -> ScriptError {
        ScriptError {
            message: message.into(),
            line,
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            f.write_str(&self.message)
        }
    }
}

impl std::error::Error for ScriptError {}

#[cfg(test)]
mod tests;
