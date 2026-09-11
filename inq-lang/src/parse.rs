use std::{collections::HashSet, fmt::Display};

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::{
    Span,
    expr::Expr,
    lex::{AnyMethod, LexError, Lexer, Lit, Method, TokenKind, TokenStream, TokenTree},
    string::IStr,
    util::{DisplayList, DisplayVec, OptionDisplay},
};

use super::lex::{GroupDelim, Keyword, Punct, TokenTreeInner};

#[derive(Debug, Clone, Error, Diagnostic)]
pub enum ParseError {
    #[error("Unexpected token {found}")]
    UnexpectedToken {
        #[label = "here"]
        span: Span,
        found: TokenTree,
    },
    #[error("Expected {}. found {}", DisplayList(expected), found)]
    UnexpectedTokenExpected {
        #[label = "here"]
        span: Span,
        found: TokenTree,
        expected: Vec<String>,
    },
    #[error("Expected {}. found EOF", DisplayList(expected))]
    UnexpectedEof { expected: Vec<String> },
    #[error("Attributes must be a single identifier")]
    InvalidAttribute {
        #[label = "here"]
        span: Span,
    },
    #[error("Unknown attribute '{}'", name)]
    UnknownAttribute {
        name: String,
        #[label = "here"]
        span: Span,
    },
}

impl ParseError {
    pub fn unexpected_eof(expected: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::UnexpectedEof {
            expected: expected.into_iter().map(Into::into).collect::<Vec<_>>(),
        }
    }
    pub fn unexpected_token(
        token: &TokenTree,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let expected = expected.into_iter().map(Into::into).collect::<Vec<_>>();
        if expected.is_empty() {
            Self::UnexpectedToken {
                span: token.span,
                found: token.clone(),
            }
        } else {
            Self::UnexpectedTokenExpected {
                span: token.span,
                found: token.clone(),
                expected,
            }
        }
    }
    pub fn unexpected(
        token: Option<TokenTree>,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        if let Some(token) = token.as_ref() {
            Self::unexpected_token(token, expected)
        } else {
            Self::unexpected_eof(expected)
        }
    }
}

pub(crate) trait Parse: Sized {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError>;
}

impl TokenStream {
    pub(crate) fn parse<P: Parse>(&mut self) -> Result<P, ParseError> {
        P::parse(self)
    }

    pub(crate) fn expect_map<F, O>(&mut self, expected: &'static str, f: F) -> Result<O, ParseError>
    where
        F: FnOnce(TokenTree) -> Option<O>,
    {
        let Some(next) = self.next() else {
            return Err(ParseError::unexpected_eof([expected]));
        };

        if let Some(o) = f(next.clone()) {
            Ok(o)
        } else {
            Err(ParseError::unexpected_token(&next, [expected]))
        }
    }

    pub(crate) fn expect_any(&mut self) -> Result<TokenTree, ParseError> {
        let Some(next) = self.next() else {
            return Err(ParseError::unexpected_eof(["Token"]));
        };

        Ok(next)
    }

    pub(crate) fn expect<K: TokenKind>(&mut self, kind: K) -> Result<TokenTree, ParseError> {
        let Some(next) = self.next() else {
            return Err(ParseError::unexpected_eof([kind.name()]));
        };

        if kind.matches(&next) {
            Ok(next)
        } else {
            Err(ParseError::unexpected_token(&next, [kind.name()]))
        }
    }

    pub(crate) fn expect_group(&mut self, delim: GroupDelim) -> Result<TokenStream, ParseError> {
        match self.expect(delim)?.inner {
            TokenTreeInner::Group { tokens, .. } => Ok(tokens),
            _ => unreachable!(),
        }
    }

    pub(crate) fn lookahead(&mut self) -> Lookahead<'_> {
        Lookahead {
            token: self.inner.front(),
            attempts: Default::default(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Lookahead<'a> {
    token: Option<&'a TokenTree>,
    attempts: Vec<&'static str>,
}

impl Lookahead<'_> {
    pub(crate) fn peek<K: TokenKind>(&mut self, k: K) -> bool {
        self.attempts.push(k.name());
        self.token.is_some_and(|tt| k.matches(tt))
    }

    pub(crate) fn eof(&mut self, expected: &'static str) -> bool {
        self.attempts.push(expected);
        self.token.is_none()
    }

    pub(crate) fn span(&self) -> Option<Span> {
        Some(self.token?.span)
    }

    pub(crate) fn error<T>(self) -> Result<T, ParseError> {
        Err(ParseError::unexpected(self.token.cloned(), self.attempts))
    }

    pub(crate) fn error_expected(
        &self,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Expr, ParseError> {
        Err(ParseError::unexpected(self.token.cloned(), expected))
    }
}

#[derive(Debug, Clone)]
pub struct Ident {
    pub(crate) inner: IStr,
    pub(crate) span: Span,
}

impl Ident {
    pub(crate) fn is_valid(s: &str) -> bool {
        let mut chars = s.chars();
        matches!(chars.next(), Some('a'..='z' | 'A'..='Z' | '_'))
            && chars.all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))
    }
}

impl AsRef<IStr> for Ident {
    fn as_ref(&self) -> &IStr {
        &self.inner
    }
}

impl Parse for Ident {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        match tokens.next() {
            Some(ref t) if let TokenTreeInner::Ident(i) = &t.inner => Ok(Self {
                inner: i.clone(),
                span: t.span,
            }),
            t => Err(ParseError::unexpected(t, ["Identifier"])),
        }
    }
}

impl Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl From<Ident> for SourceSpan {
    fn from(value: Ident) -> Self {
        value.span.into()
    }
}

#[derive(Clone, Debug)]
pub struct Interpolation {
    pub(crate) index: usize,
    pub(crate) expr: Expr,
}

#[derive(Clone, Debug)]
pub struct StringExpr {
    pub(crate) span: Span,
    pub(crate) value: IStr,
    pub(crate) interpolations: Vec<Interpolation>,
}

impl Display for StringExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut last = 0;
        write!(f, "\"")?;
        for e in &self.interpolations {
            write!(f, "{}", &self.value[last..e.index])?;
            write!(f, "${{{}}}", e.expr)?;
            last = e.index;
        }
        if !self.value[last..].is_empty() {
            write!(f, "{:?}", &self.value[last..])?;
        }
        write!(f, "\"")?;
        Ok(())
    }
}

impl Parse for StringExpr {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        let (span, value, interpolations) =
            tokens.expect_map("String literal", |tt| match tt.inner {
                TokenTreeInner::Literal(Lit::String {
                    value,
                    interpolations,
                }) => Some((tt.span, value, interpolations)),
                _ => None,
            })?;

        Ok(Self {
            span,
            value,
            interpolations: interpolations
                .into_iter()
                .map(|mut t| {
                    Ok(Interpolation {
                        index: t.index,
                        expr: t.tokens.parse()?,
                    })
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone, derive_more::Display)]
#[display("{}", DisplayVec(exprs))]
pub struct Block {
    pub(crate) exprs: Vec<Expr>,
    /// Whether the last expression should be evaluated as a return statement
    pub(crate) ret: bool,
    pub(crate) span: Span,
}

impl Parse for Block {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        let mut tokens = tokens.expect_group(GroupDelim::Brace)?;
        let mut exprs = Vec::new();
        let mut ret = true;
        loop {
            while tokens.next_if(Punct::Semicolon).is_some() {}

            if tokens.is_empty() {
                break;
            }

            exprs.push(tokens.parse()?);

            let mut la = tokens.lookahead();
            if la.peek(Punct::Semicolon) {
                tokens.expect(Punct::Semicolon)?;
                if tokens.is_empty() {
                    ret = false;
                    break;
                }
            } else if la.eof("End of block") {
                break;
            } else {
                return la.error();
            }
        }

        Ok(Self {
            exprs,
            ret,
            span: tokens.span(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Attribute {
    Persist,
}

impl Parse for Attribute {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        if tokens.len() != 1 {
            return Err(ParseError::InvalidAttribute {
                span: tokens.span(),
            });
        }

        let token = tokens.next().unwrap();
        match token.inner {
            TokenTreeInner::Ident(ident) => match &*ident {
                "persist" => Ok(Self::Persist),
                _ => Err(ParseError::UnknownAttribute {
                    name: ident.into(),
                    span: token.span,
                }),
            },
            _ => Err(ParseError::InvalidAttribute { span: token.span }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Path(Vec<Ident>);

impl Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, s) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{}", s)?;
        }
        Ok(())
    }
}

impl Parse for Path {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        let mut path = Vec::new();
        loop {
            let ident = tokens.parse()?;
            path.push(ident);
            if tokens.next_if(Punct::Slash).is_none() {
                break;
            }
        }
        Ok(Self(path))
    }
}

#[derive(Clone, Debug)]
pub struct RouteArg {
    pub name: Ident,
    pub default_value: Option<Expr>,
}

#[derive(derive_more::Debug, Clone)]
pub struct Route {
    pub name: Path,
    pub args: Vec<RouteArg>,
    pub method: Method,
    #[debug("{}", endpoint)]
    pub endpoint: Expr,
    #[debug("{}", OptionDisplay(before))]
    pub before: Option<Block>,
    #[debug("{}", OptionDisplay(before))]
    pub after: Option<Block>,
}

impl Parse for Route {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        tokens.expect(Keyword::Route)?;
        let path: Path = tokens.parse()?;
        let mut la = tokens.lookahead();

        let args = if la.peek(GroupDelim::Paren) {
            let group = tokens.expect_group(GroupDelim::Paren)?;
            let args = Self::parse_args(group)?;
            tokens.expect(Punct::Arrow)?;
            args
        } else if la.peek(Punct::Arrow) {
            tokens.expect(Punct::Arrow)?;
            Default::default()
        } else {
            return la.error();
        };

        let TokenTreeInner::Keyword(Keyword::Method(method)) = tokens.expect(AnyMethod)?.inner
        else {
            unreachable!()
        };

        let endpoint: Expr = tokens.parse()?;

        let mut la = tokens.lookahead();
        let (before, after) = if la.peek(Punct::Semicolon) {
            let _ = tokens.expect(Punct::Semicolon)?;
            (None, None)
        } else if la.peek(Keyword::Before) {
            let _ = tokens.expect(Keyword::Before)?;
            let before = tokens.parse()?;

            let after = tokens
                .next_if(Keyword::After)
                .map(|_| tokens.parse())
                .transpose()?;

            (Some(before), after)
        } else if la.peek(Keyword::After) {
            let _ = tokens.expect(Keyword::After)?;
            let after = tokens.parse()?;

            (None, Some(after))
        } else {
            return la.error();
        };

        Ok(Self {
            name: path,
            args,
            method,
            endpoint,
            before,
            after,
        })
    }
}

impl Route {
    fn parse_args(mut tokens: TokenStream) -> Result<Vec<RouteArg>, ParseError> {
        let mut out = Vec::new();

        loop {
            if tokens.is_empty() {
                break;
            }
            let name = tokens.parse()?;

            let mut la = tokens.lookahead();
            let default = if la.peek(Punct::Eq) {
                let _eq = tokens.next().unwrap();
                Some(tokens.parse()?)
            } else if la.peek(Punct::Comma) {
                let _comma = tokens.next().unwrap();
                None
            } else if la.eof("End of args") {
                None
            } else {
                return la.error();
            };

            out.push(RouteArg {
                name,
                default_value: default,
            })
        }

        Ok(out)
    }
}

#[derive(Clone, Debug)]
pub struct Variable {
    pub name: Ident,
    pub value: Expr,
    pub attributes: HashSet<Attribute>,
}

impl Parse for Variable {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        tokens.expect(Keyword::Let)?;
        let name = tokens.parse()?;
        let _eq = tokens.expect(Punct::Eq)?;
        let value: Expr = tokens.parse()?;
        let _semi = tokens.expect(Punct::Semicolon)?;
        Ok(Self {
            name,
            value,
            attributes: Default::default(),
        })
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    Variable(Variable),
    Route(Route),
}

#[derive(Debug, Clone)]
pub struct Parser {
    tokens: TokenStream,
}

impl Parser {
    pub fn new(lexer: Lexer<'_>) -> Result<Self, LexError> {
        Ok(Self {
            tokens: lexer.all()?,
        })
    }

    pub fn take_item_inner(
        &mut self,
        mut attributes: HashSet<Attribute>,
    ) -> Result<Option<Item>, ParseError> {
        if self.tokens.is_empty() {
            return Ok(None);
        };

        let mut la = self.tokens.lookahead();

        let item = if la.peek(Keyword::Let) {
            Item::Variable(Variable {
                attributes,
                ..self.tokens.parse()?
            })
        } else if la.peek(Keyword::Route) {
            Item::Route(self.tokens.parse()?)
        } else if la.peek(Punct::Hash) {
            self.tokens.expect(Punct::Hash)?;

            let mut attr = self.tokens.expect_group(GroupDelim::Bracket)?;

            attributes.insert(attr.parse()?);

            let Some(item) = self.take_item_inner(attributes)? else {
                return Err(ParseError::unexpected_eof(["Item"]));
            };
            item
        } else {
            return la.error();
        };

        Ok(Some(item))
    }

    pub fn take_item(&mut self) -> Result<Option<Item>, ParseError> {
        self.take_item_inner(Default::default())
    }

    pub fn take_expr(&mut self) -> Result<Option<Expr>, ParseError> {
        while self.tokens.next_if(Punct::Semicolon).is_some() {}
        if self.tokens.is_empty() {
            return Ok(None);
        }
        let e: Expr = self.tokens.parse()?;
        Ok(Some(e))
    }
}
