use std::{
    collections::VecDeque,
    fmt::Display,
    num::{ParseFloatError, ParseIntError},
};

use miette::Diagnostic;
use phf::phf_map;
use thiserror::Error;

use crate::{IStr, Span};

pub trait TokenKind {
    fn matches(&self, tt: &TokenTree) -> bool;
    fn name(&self) -> &'static str;
}

impl<K: TokenKind> TokenKind for &K {
    fn matches(&self, tt: &TokenTree) -> bool {
        (*self).matches(tt)
    }

    fn name(&self) -> &'static str {
        (*self).name()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupDelim {
    /// `()`
    Paren,
    /// `{}`
    Brace,
    /// `[]`
    Bracket,
}

impl TokenKind for GroupDelim {
    fn matches(&self, tt: &TokenTree) -> bool {
        matches!(&tt.inner, TokenTreeInner::Group { delim, .. } if delim == self)
    }

    fn name(&self) -> &'static str {
        match self {
            GroupDelim::Paren => "`(`",
            GroupDelim::Brace => "`{`",
            GroupDelim::Bracket => "`[`",
        }
    }
}

macro_rules! impl_punct {
    ($($ident: ident => $symbol: literal (op: $op: literal))*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Punct {
            $(
                #[doc = concat!("`", $symbol, "`")]
                $ident
            ),*
        }

        impl Punct {
            fn from_string(s: &str) -> Option<Self> {
                $(
                    if s.starts_with($symbol) {
                        return Some(Self::$ident);
                    }
                )*
                None
            }

            const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$ident => $symbol),*
                }
            }

            pub(crate) const fn is_op(self) -> bool {
                match self {
                    $(Self::$ident => $op),*
                }
            }
        }

        impl TokenKind for Punct {
            fn matches(&self, tt: &TokenTree) -> bool {
                matches!(&tt.inner, TokenTreeInner::Punct(p) if p == self)
            }

            fn name(&self) -> &'static str {
                match self {
                    $(Self::$ident => concat!("`", $symbol, "`")),*
                }
            }
        }

    };
}

// NOTE: Symbols with a common prefix must have the longest items declared first
impl_punct! {
    Arrow     => "=>" (op: true  )
    EqEq      => "==" (op: true  )
    Eq        => "="  (op: true  )
    LtEq      => "<=" (op: true  )
    Lt        => "<"  (op: true  )
    GtEq      => ">=" (op: true  )
    Gt        => ">"  (op: true  )
    BangEq    => "!=" (op: true  )
    Semicolon => ";"  (op: false )
    PipePipe  => "||" (op: true  )
    AndAnd    => "&&" (op: true  )
    Hash      => "#"  (op: false )
    Dot       => "."  (op: true  )
    Comma     => ","  (op: false )
    Slash     => "/"  (op: true  )
    Colon     => ":"  (op: false )
    Plus      => "+"  (op: true  )
    Minus     => "-"  (op: true  )
    Bang      => "!"  (op: true  )
    Star      => "*"  (op: true  )
}

impl Display for Punct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnyMethod;

impl TokenKind for AnyMethod {
    fn matches(&self, tt: &TokenTree) -> bool {
        matches!(&tt.inner, TokenTreeInner::Keyword(Keyword::Method(_)))
    }

    fn name(&self) -> &'static str {
        "Method"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display)]
#[display("{}", self.as_str())]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Options,
    Trace,
    Patch,
}

impl Method {
    const fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Options => "OPTIONS",
            Method::Trace => "TRACE",
            Method::Patch => "PATCH",
        }
    }
}

impl TokenKind for Method {
    fn matches(&self, tt: &TokenTree) -> bool {
        if let TokenTreeInner::Keyword(kw) = &tt.inner
            && let Keyword::Method(m) = kw
            && m == self
        {
            true
        } else {
            false
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Method::Get => "`GET`",
            Method::Head => "`HEAD`",
            Method::Post => "`POST`",
            Method::Put => "`PUT`",
            Method::Delete => "`DELETE`",
            Method::Options => "`OPTIONS`",
            Method::Trace => "`TRACE`",
            Method::Patch => "`PATCH`",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyword {
    Let,
    If,
    Then,
    Else,
    Route,
    Before,
    Request,
    After,
    Response,
    True,
    False,
    Null,
    Method(Method),
}

impl Keyword {
    fn from_str(s: &str) -> Option<Self> {
        phf_map! {
            "let"      => Keyword::Let,
            "if"       => Keyword::If,
            "then"     => Keyword::Then,
            "else"     => Keyword::Else,
            "route"    => Keyword::Route,
            "before"   => Keyword::Before,
            "request"  => Keyword::Request,
            "after"    => Keyword::After,
            "response" => Keyword::Response,
            "true"     => Keyword::True,
            "false"    => Keyword::False,
            "null"     => Keyword::Null,
            "GET"      => Keyword::Method(Method::Get),
            "HEAD"     => Keyword::Method(Method::Head),
            "POST"     => Keyword::Method(Method::Post),
            "PUT"      => Keyword::Method(Method::Put),
            "DELETE"   => Keyword::Method(Method::Delete),
            "OPTIONS"  => Keyword::Method(Method::Options),
            "TRACE"    => Keyword::Method(Method::Trace),
            "PATCH"    => Keyword::Method(Method::Patch),
        }
        .get(s)
        .copied()
    }
}

impl Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
impl TokenKind for Keyword {
    fn matches(&self, tt: &TokenTree) -> bool {
        matches!(&tt.inner, TokenTreeInner::Keyword(kw) if kw == self)
    }

    fn name(&self) -> &'static str {
        match self {
            Keyword::Let => "`let`",
            Keyword::If => "`if`",
            Keyword::Then => "`then`",
            Keyword::Else => "`else`",
            Keyword::Route => "`route`",
            Keyword::Before => "`before`",
            Keyword::Request => "`request`",
            Keyword::After => "`after`",
            Keyword::Response => "`response`",
            Keyword::True => "`true`",
            Keyword::False => "`false`",
            Keyword::Null => "`null`",
            Keyword::Method(m) => m.name(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Interpolation {
    pub(crate) index: usize,
    pub(crate) tokens: TokenStream,
}

#[derive(Clone, Debug)]
pub(crate) enum Lit {
    String {
        value: IStr,
        interpolations: Vec<Interpolation>,
    },
    Int(u64),
    Float(f64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LitKind {
    String,
    Int,
    Float,
}

impl TokenKind for LitKind {
    fn matches(&self, tt: &TokenTree) -> bool {
        matches!(
            (&tt.inner, self),
            (TokenTreeInner::Literal(Lit::String { .. }), Self::String)
                | (TokenTreeInner::Literal(Lit::Int(_)), Self::Int)
                | (TokenTreeInner::Literal(Lit::Float(_)), Self::Float)
        )
    }

    fn name(&self) -> &'static str {
        match self {
            LitKind::String => "String",
            LitKind::Int => "Integer",
            LitKind::Float => "Float",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TokenStream {
    pub(crate) inner: VecDeque<TokenTree>,
    span: Span,
}

impl TokenStream {
    fn single(tt: TokenTree) -> TokenStream {
        TokenStream {
            span: tt.span,
            inner: VecDeque::from_iter([tt]),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn next(&mut self) -> Option<TokenTree> {
        self.inner.pop_front()
    }

    pub fn next_if(&mut self, kind: impl TokenKind) -> Option<TokenTree> {
        let peek = self.peek()?;
        if kind.matches(peek) {
            self.next()
        } else {
            None
        }
    }

    pub fn peek(&self) -> Option<&TokenTree> {
        self.inner.front()
    }

    pub fn peek_kind(&self, kind: impl TokenKind) -> Option<&TokenTree> {
        let peek = self.peek()?;
        kind.matches(peek).then_some(peek)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TokenTreeInner {
    Group {
        delim: GroupDelim,
        tokens: TokenStream,
    },
    Ident(IStr),
    Punct(Punct),
    Keyword(Keyword),
    Literal(Lit),
}

impl Display for TokenTreeInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenTreeInner::Group { .. } => write!(f, "Group"),
            TokenTreeInner::Ident(_) => write!(f, "Identifier"),
            TokenTreeInner::Punct(p) => write!(f, "{}", p),
            TokenTreeInner::Keyword(kw) => write!(f, "Keyword {}", kw),
            TokenTreeInner::Literal(lit) => match lit {
                Lit::String { .. } => write!(f, "String literal"),
                Lit::Int(_) => write!(f, "Integer literal"),
                Lit::Float(_) => write!(f, "Float literal"),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenTree {
    pub(crate) inner: TokenTreeInner,
    pub(crate) span: Span,
}

impl Display for TokenTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

#[derive(Debug, Clone, Error, Diagnostic)]
pub enum LexError {
    #[error("Expected {}, got EOF", _0)]
    UnexpectedEof(String),
    #[error("Unexpected Character '{}'", actual)]
    UnexpectedCharacter {
        actual: char,
        #[label = "here"]
        position: Span,
    },
    #[error("Invalid integer literal: {}", source)]
    InvalidInteger {
        #[from]
        source: ParseIntError,
    },
    #[error("Invalid float literal: {}", source)]
    InvalidFloat {
        #[from]
        source: ParseFloatError,
    },
    #[error("Float exponent must have at least one digit")]
    ExpectedFloatExponent {
        #[label = "here"]
        span: Span,
    },
}

pub struct Lexer<'a> {
    content: &'a str,
    position: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(content: &'a str) -> Self {
        Lexer {
            content,
            position: 0,
        }
    }

    pub(crate) fn all(mut self) -> Result<TokenStream, LexError> {
        let start = self.position;
        let mut inner = VecDeque::new();

        while let Some(t) = self.next_tt()? {
            inner.push_back(t);
        }

        Ok(TokenStream {
            inner,
            span: (start..self.position).into(),
        })
    }

    fn skip_whitespace(&mut self) {
        let rest = &self.content[self.position..];
        let idx = rest
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(rest.len());
        self.position += idx;
    }

    fn peek_char(&self) -> Option<char> {
        self.content[self.position..].chars().next()
    }

    fn take_char(&mut self) -> Option<char> {
        let c = self.content[self.position..].chars().next()?;
        self.position += c.len_utf8();
        Some(c)
    }

    fn expect_char(&mut self) -> Result<char, LexError> {
        self.take_char()
            .ok_or_else(|| LexError::UnexpectedEof("token".into()))
    }

    fn untake_char(&mut self, c: char) {
        self.position -= c.len_utf8();
        let actual = self.content[self.position..].chars().next();
        assert_eq!(actual, Some(c));
    }

    fn take_ident(&mut self) -> Result<TokenTree, LexError> {
        let start = self.position;
        while let Some('a'..='z' | 'A'..='Z' | '_' | '0'..='9') = self.peek_char() {
            self.take_char();
        }

        let ident = &self.content[start..self.position];
        assert!(!ident.is_empty());

        if let Some(kw) = Keyword::from_str(ident) {
            Ok(TokenTree {
                inner: TokenTreeInner::Keyword(kw),
                span: (start..self.position).into(),
            })
        } else {
            Ok(TokenTree {
                inner: TokenTreeInner::Ident(ident.into()),
                span: (start..self.position).into(),
            })
        }
    }

    fn take_number(&mut self) -> Result<TokenTreeInner, LexError> {
        let start = self.position;
        let mut has_dot = false;
        let mut is_float = false;
        let mut has_e = false;
        while let Some(c) = self.take_char() {
            match c {
                '0'..='9' => {}
                '.' if !has_dot => {
                    // if dot is followed by identifier, make a it be [int] [dot] [ident]
                    {
                        let rest = self.content[self.position..].trim_start();
                        if matches!(rest.chars().next(), Some('a'..='z' | 'A'..='Z' | '_')) {
                            self.untake_char('.');
                            break;
                        }
                    }
                    has_dot = true;
                    is_float = true;
                }
                'e' if !has_e => {
                    match self.take_char() {
                        Some('0'..='9') => {}
                        Some('-') if let Some('0'..='9') = self.take_char() => {}
                        _ => {
                            return Err(LexError::ExpectedFloatExponent {
                                span: (start..self.position - 1).into(),
                            });
                        }
                    }
                    has_dot = true;
                    is_float = true;
                    has_e = true;
                }
                c => {
                    self.untake_char(c);
                    break;
                }
            }
        }

        let ident = &self.content[start..self.position];
        let n = if is_float {
            Lit::Float(ident.parse::<f64>()?)
        } else {
            Lit::Int(ident.parse::<u64>()?)
        };
        Ok(TokenTreeInner::Literal(n))
    }

    fn take_string(&mut self) -> Result<TokenTreeInner, LexError> {
        let mut value = String::new();
        let mut interpolations = Vec::new();

        let mut escaping = false;

        loop {
            match self.expect_char()? {
                '"' if !escaping => break,
                '\\' if !escaping => {
                    escaping = true;
                }
                '$' if !escaping => match self.take_char() {
                    Some(c @ ('a'..='z' | 'A'..='Z' | '_')) => {
                        self.untake_char(c);
                        let tt = self.take_ident()?;
                        interpolations.push(Interpolation {
                            index: value.len(),
                            tokens: TokenStream::single(tt),
                        });
                    }
                    Some('{') => {
                        let tokens = self.take_group('}')?;
                        interpolations.push(Interpolation {
                            index: value.len(),
                            tokens,
                        });
                    }
                    Some(c) => {
                        return Err(LexError::UnexpectedCharacter {
                            actual: c,
                            position: (self.position..self.position + c.len_utf8()).into(),
                        });
                    }
                    None => {
                        return Err(LexError::UnexpectedEof("Group or Ident".into()));
                    }
                },
                c => {
                    value.push(c);
                    escaping = false;
                }
            }
        }

        Ok(TokenTreeInner::Literal(Lit::String {
            value: value.into(),
            interpolations,
        }))
    }

    fn take_comment(&mut self) {
        let rest = &self.content[self.position..];
        self.position += rest.find('\n').unwrap_or(rest.len());
    }

    fn take_group(&mut self, close: char) -> Result<TokenStream, LexError> {
        let start = self.position - 1;
        let mut tokens = VecDeque::new();
        loop {
            self.skip_whitespace();
            let c = self.expect_char()?;
            if c == close {
                break;
            }

            self.untake_char(c);

            tokens.push_back(self.next_tt()?.expect("checked by expect_char"));
        }
        Ok(TokenStream {
            inner: tokens,
            span: (start..self.position).into(),
        })
    }

    pub(crate) fn next_tt(&mut self) -> Result<Option<TokenTree>, LexError> {
        self.skip_whitespace();

        let start = self.position;

        let Some(c) = self.take_char() else {
            return Ok(None);
        };

        let tt = match (c, self.peek_char()) {
            ('/', Some('/')) => {
                self.take_comment();
                return self.next_tt();
            }
            ('(', _) => TokenTreeInner::Group {
                delim: GroupDelim::Paren,
                tokens: self.take_group(')')?,
            },
            ('{', _) => TokenTreeInner::Group {
                delim: GroupDelim::Brace,
                tokens: self.take_group('}')?,
            },
            ('[', _) => TokenTreeInner::Group {
                delim: GroupDelim::Bracket,
                tokens: self.take_group(']')?,
            },
            (c @ ('a'..='z' | 'A'..='Z' | '_'), _) => {
                self.untake_char(c);
                return self.take_ident().map(Some);
            }
            (c @ ('0'..='9'), _) => {
                self.untake_char(c);
                self.take_number()?
            }
            ('"', _) => self.take_string()?,
            (c, _) => {
                self.untake_char(c);

                if let Some(p) = Punct::from_string(&self.content[self.position..]) {
                    self.position += p.as_str().len();
                    TokenTreeInner::Punct(p)
                } else {
                    return Err(LexError::UnexpectedCharacter {
                        actual: c,
                        position: (self.position..self.position + c.len_utf8()).into(),
                    });
                }
            }
        };

        Ok(Some(TokenTree {
            inner: tt,
            span: (start..self.position).into(),
        }))
    }
}
