//! Rich diagnostics for the NEPL compiler.
//!
//! This module defines diagnostic structures used to report errors
//! and warnings with precise source locations and optional notes.

use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;

/// Severity level of a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A labeled span used inside diagnostics.
///
/// Each label points to a specific span in the source code and
/// optionally carries a short message explaining the highlight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

/// A single diagnostic message produced by the compiler.
///
/// A diagnostic has a main message, a primary label indicating the
/// main source location, and zero or more secondary labels for
/// related locations (for example, “defined here”, “required here”).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<&'static str>,
    pub message: String,
    pub primary: Label,
    pub secondary: Vec<Label>,
}

impl Diagnostic {
    /// Create a new error diagnostic with a primary span.
    pub fn error(message: impl Into<String>, primary_span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            primary: Label {
                span: primary_span,
                message: None,
            },
            secondary: Vec::new(),
        }
    }

    /// Create a new warning diagnostic with a primary span.
    pub fn warning(message: impl Into<String>, primary_span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            code: None,
            message: message.into(),
            primary: Label {
                span: primary_span,
                message: None,
            },
            secondary: Vec::new(),
        }
    }

    /// Attach an error code (for example, "E0001") to this diagnostic.
    pub fn with_code(mut self, code: &'static str) -> Diagnostic {
        self.code = Some(code);
        self
    }

    /// Add a secondary label with its own span and optional message.
    pub fn with_secondary_label(
        mut self,
        span: Span,
        message: impl Into<Option<String>>,
    ) -> Diagnostic {
        self.secondary.push(Label {
            span,
            message: message.into(),
        });
        self
    }
}
