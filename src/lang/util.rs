use std::{
    fmt::{Debug, Display},
    ops::{Add, Range},
};

use miette::SourceSpan;

#[derive(Clone, Copy)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub(crate) fn empty() -> Self {
        Self { start: 0, end: 0 }
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

#[derive(Debug, Clone)]
pub(crate) struct DisplayList<D: Display>(pub(crate) Vec<D>);
impl<D: Display> Display for DisplayList<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self.0 {
            [] => write!(f, "token"),
            [a] => write!(f, "{a}"),
            [a, b] => write!(f, "one of {a} or {b}"),
            expected => {
                write!(f, "one of ")?;
                for (i, e) in expected.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    if i == expected.len() - 1 {
                        write!(f, "or ")?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DisplayVec<T>(pub Vec<T>);
impl<T: Display> Display for DisplayVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", item)?;
        }
        write!(f, "]")
    }
}
