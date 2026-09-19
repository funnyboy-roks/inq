use std::{
    fmt::Debug,
    ops::{Add, Range},
};

use miette::{LabeledSpan, SourceSpan};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn empty() -> Self {
        Self { start: 0, end: 0 }
    }

    pub fn with_label(self, label: impl Into<String>) -> LabeledSpan {
        LabeledSpan::new_with_span(Some(label.into()), self)
    }
}

impl From<Span> for SourceSpan {
    fn from(value: Span) -> Self {
        SourceSpan::new(value.start.into(), value.end - value.start)
    }
}

impl From<Range<usize>> for Span {
    fn from(value: Range<usize>) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

impl Add for Span {
    type Output = Span;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            start: self.start.min(rhs.start),
            end: self.end.max(rhs.end),
        }
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
