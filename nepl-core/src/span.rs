//! Source code span utilities.
//!
//! This module defines file identifiers and byte-range spans used
//! for diagnostics and error reporting. All tokens and AST/HIR nodes
//! are expected to be associated with a Span in later phases.

/// Identifier for a source file.
///
/// In the simplest setup, this can be assigned incrementally as
/// files are loaded. The actual mapping from `FileId` to a path or
/// source text is maintained by higher-level components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// A half-open byte range `[start, end)` within a given file.
///
/// Positions are expressed in bytes relative to the file content,
/// not in character indices or line/column. Line/column information
/// can be derived separately if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Construct a new span for the given file and byte range.
    pub fn new(file_id: FileId, start: u32, end: u32) -> Span {
        Span { file_id, start, end }
    }

    /// Construct an empty span at the given position.
    pub fn empty(file_id: FileId, pos: u32) -> Span {
        Span {
            file_id,
            start: pos,
            end: pos,
        }
    }

    /// Returns the length in bytes of this span.
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if this span has zero length.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns a span that covers both `self` and `other`, if they
    /// belong to the same file. Otherwise returns `None`.
    pub fn join(self, other: Span) -> Option<Span> {
        if self.file_id != other.file_id {
            return None;
        }
        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        Some(Span::new(self.file_id, start, end))
    }

    /// A placeholder span for situations where no precise source
    /// location is available yet.
    ///
    /// This is useful while incrementally migrating the compiler to
    /// span-aware diagnostics.
    pub fn dummy() -> Span {
        Span {
            file_id: FileId(0),
            start: 0,
            end: 0,
        }
    }
}
