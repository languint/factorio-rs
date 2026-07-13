use std::ops::Range;

/// Inclusive-exclusive byte range into a Rust source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

impl From<Range<usize>> for SourceSpan {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

/// A span plus optional human-readable detail (e.g. expression kind).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceLoc {
    pub span: SourceSpan,
    pub note: Option<String>,
}

impl SourceLoc {
    #[must_use]
    pub fn new(span: impl Into<SourceSpan>) -> Self {
        Self {
            span: span.into(),
            note: None,
        }
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

impl std::fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.span.start, self.span.end)?;
        if let Some(note) = &self.note {
            write!(f, " ({note})")?;
        }
        Ok(())
    }
}
