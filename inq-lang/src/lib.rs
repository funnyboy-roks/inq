pub mod eval;
pub mod expr;
mod lex;
mod parse;
mod span;
mod string;
pub(crate) mod util;

#[cfg(test)]
mod test;
#[cfg(any(feature = "testing", test))]
pub mod testing;

pub use lex::Method;
use miette::NamedSource;
pub use parse::{Attribute, Ident, Item, Parser, Path, Route, RouteArg, Variable};
pub use span::Span;
pub use string::{IStr, StringExt};

pub fn source(name: impl Into<String>, source: impl Into<String>) -> NamedSource<String> {
    NamedSource::new(name.into(), source.into()).with_language("inq")
}
