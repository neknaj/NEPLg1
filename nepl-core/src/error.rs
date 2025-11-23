//! Core error type for the NEPL language toolchain.
//!
//! This module provides a minimal error type that is compatible
//! with `no_std`. Language-level errors should be expressed as
//! `Diagnostic` values; `CoreError` is the outer error wrapper
//! used by the core compiler pipeline.

use core::fmt;

use alloc::vec::Vec;

use crate::diagnostic::Diagnostic;

/// Core error type for the nepl-core crate.
///
/// High-level tools (CLI, web, etc.) are expected to:
///   - handle I/O and environment errors on their side, and
///   - render `Diagnostic` values for language-level errors.
#[derive(Debug, Clone)]
pub enum CoreError {
    /// One or more language-level errors with full diagnostic
    /// information (spans, labels, codes, etc.).
    Diagnostics(Vec<Diagnostic>),

    /// An internal error indicating a bug in the compiler or an
    /// unexpected unreachable situation.
    ///
    /// This variant is not intended for user-facing error messages,
    /// but can be useful during development and debugging.
    Internal(&'static str),
}

impl CoreError {
    /// Construct a CoreError from a single Diagnostic.
    pub fn from_diagnostic(diagnostic: Diagnostic) -> CoreError {
        CoreError::Diagnostics(vec![diagnostic])
    }

    /// Construct a CoreError from multiple Diagnostics.
    pub fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> CoreError {
        CoreError::Diagnostics(diagnostics)
    }

    /// Construct an internal error with a static message.
    pub fn internal(message: &'static str) -> CoreError {
        CoreError::Internal(message)
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Diagnostics(diags) => {
                if let Some(first) = diags.first() {
                    // とりあえず先頭のメッセージだけを出す。
                    // 上位レイヤーで複数の Diagnostic を整形表示することを想定している。
                    write!(f, "{}", first.message)
                } else {
                    write!(f, "diagnostic error (no messages)")
                }
            }
            CoreError::Internal(msg) => write!(f, "internal compiler error: {msg}"),
        }
    }
}
