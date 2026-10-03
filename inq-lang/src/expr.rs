use std::fmt::Display;

use crate::{
    IStr, Span,
    eval::value::ValueRef,
    lex::{self, GroupDelim, Keyword, LitKind, Punct, TokenKind, TokenStream, TokenTree},
    parse::{Block, Ident, Parse, ParseError, StringExpr},
    util::{DisplayList, DisplayVec},
};

use super::lex::TokenTreeInner;

struct Operator;
impl TokenKind for Operator {
    fn matches(&self, tt: &TokenTree) -> bool {
        match &tt.inner {
            TokenTreeInner::Punct(p) => p.is_op(),
            TokenTreeInner::Group { .. }
            | TokenTreeInner::Ident(_)
            | TokenTreeInner::Keyword(_)
            | TokenTreeInner::Literal(_) => false,
        }
    }

    fn name(&self) -> &'static str {
        "Operator"
    }
}

struct Identifier;
impl TokenKind for Identifier {
    fn matches(&self, tt: &TokenTree) -> bool {
        matches!(tt.inner, TokenTreeInner::Ident(_))
    }

    fn name(&self) -> &'static str {
        "Identifier"
    }
}

#[derive(Clone, Copy, Debug, derive_more::Display)]
pub enum Lit {
    #[display("{_0}")]
    Int(u64),
    #[display("{_0}")]
    Float(f64),
    #[display("{_0}")]
    Bool(bool),
    #[display("null")]
    Null,
}

#[derive(Clone, Copy, Debug, derive_more::Display)]
pub enum PrefixOp {
    #[display("-")]
    Neg,
    #[display("!")]
    Not,
}

impl PrefixOp {
    fn from_tt(op: &TokenTree) -> Option<Self> {
        match op.inner {
            TokenTreeInner::Punct(Punct::Minus) => Some(Self::Neg),
            TokenTreeInner::Punct(Punct::Bang) => Some(Self::Not),
            _ => None,
        }
    }

    fn bp(self) -> ((), u8) {
        match self {
            PrefixOp::Neg | PrefixOp::Not => ((), 5),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display)]
pub enum CmpOp {
    /// `<`
    #[display("<")]
    Lt,
    /// `<=`
    #[display("<=")]
    Lte,
    /// `>`
    #[display(">")]
    Gt,
    /// `>=`
    #[display(">=")]
    Gte,
    /// `==`
    #[display("==")]
    Eq,
    /// `!=`
    #[display("!=")]
    NotEq,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display)]
pub enum InfixOp {
    #[display("{_0}")]
    Cmp(CmpOp),
    #[display("=")]
    Assign,
    #[display("||")]
    Or,
    #[display("||=")]
    OrAssign,
    #[display("&&")]
    And,
    #[display("&&=")]
    AndAssign,
    #[display("+")]
    Add,
    #[display("+=")]
    AddAssign,
    #[display("-")]
    Sub,
    #[display("-=")]
    SubAssign,
    #[display("*")]
    Mul,
    #[display("*=")]
    MulAssign,
    #[display("/")]
    Div,
    #[display("/=")]
    DivAssign,
    #[display("%")]
    Mod,
    #[display("%=")]
    ModAssign,
    #[display("&&")]
    BitAnd,
    #[display("&&=")]
    BitAndAssign,
    #[display("|")]
    BitOr,
    #[display("|=")]
    BitOrAssign,
    #[display("^")]
    Xor,
    #[display("^=")]
    XorAssign,
    #[display("<<")]
    Shl,
    #[display("<<=")]
    ShlAssign,
    #[display(">>")]
    Shr,
    #[display(">>=")]
    ShrAssign,
}

impl InfixOp {
    fn from_tt(op: &TokenTree) -> Option<Self> {
        match op.inner {
            TokenTreeInner::Punct(Punct::Eq) => Some(Self::Assign),
            TokenTreeInner::Punct(Punct::EqEq) => Some(Self::Cmp(CmpOp::Eq)),
            TokenTreeInner::Punct(Punct::BangEq) => Some(Self::Cmp(CmpOp::NotEq)),
            TokenTreeInner::Punct(Punct::Lt) => Some(Self::Cmp(CmpOp::Lt)),
            TokenTreeInner::Punct(Punct::LtEq) => Some(Self::Cmp(CmpOp::Lte)),
            TokenTreeInner::Punct(Punct::Gt) => Some(Self::Cmp(CmpOp::Gt)),
            TokenTreeInner::Punct(Punct::GtEq) => Some(Self::Cmp(CmpOp::Gte)),
            // TokenTreeInner::Punct(Punct::DotDot | Punct::DotDotEq) => Some((4, 5)),
            TokenTreeInner::Punct(Punct::Plus) => Some(Self::Add),
            TokenTreeInner::Punct(Punct::PlusEq) => Some(Self::AddAssign),
            TokenTreeInner::Punct(Punct::Minus) => Some(Self::Sub),
            TokenTreeInner::Punct(Punct::MinusEq) => Some(Self::SubAssign),
            TokenTreeInner::Punct(Punct::Star) => Some(Self::Mul),
            TokenTreeInner::Punct(Punct::StarEq) => Some(Self::MulAssign),
            TokenTreeInner::Punct(Punct::Slash) => Some(Self::Div),
            TokenTreeInner::Punct(Punct::SlashEq) => Some(Self::DivAssign),
            TokenTreeInner::Punct(Punct::PipePipe) => Some(Self::Or),
            TokenTreeInner::Punct(Punct::PipePipeEq) => Some(Self::OrAssign),
            TokenTreeInner::Punct(Punct::AndAnd) => Some(Self::And),
            TokenTreeInner::Punct(Punct::AndAndEq) => Some(Self::AndAssign),
            TokenTreeInner::Punct(Punct::Percent) => Some(Self::Mod),
            TokenTreeInner::Punct(Punct::PercentEq) => Some(Self::ModAssign),
            TokenTreeInner::Punct(Punct::And) => Some(Self::BitAnd),
            TokenTreeInner::Punct(Punct::AndEq) => Some(Self::BitAndAssign),
            TokenTreeInner::Punct(Punct::Pipe) => Some(Self::BitOr),
            TokenTreeInner::Punct(Punct::PipeEq) => Some(Self::BitOrAssign),
            TokenTreeInner::Punct(Punct::Caret) => Some(Self::Xor),
            TokenTreeInner::Punct(Punct::CaretEq) => Some(Self::XorAssign),
            TokenTreeInner::Punct(Punct::LtLt) => Some(Self::Shl),
            TokenTreeInner::Punct(Punct::LtLtEq) => Some(Self::ShlAssign),
            TokenTreeInner::Punct(Punct::GtGt) => Some(Self::Shr),
            TokenTreeInner::Punct(Punct::GtGtEq) => Some(Self::ShrAssign),
            _ => None,
        }
    }

    fn bp(self) -> (u8, u8) {
        match self {
            InfixOp::OrAssign
            | InfixOp::AndAssign
            | InfixOp::AddAssign
            | InfixOp::SubAssign
            | InfixOp::MulAssign
            | InfixOp::DivAssign
            | InfixOp::ModAssign
            | InfixOp::BitAndAssign
            | InfixOp::BitOrAssign
            | InfixOp::XorAssign
            | InfixOp::ShlAssign
            | InfixOp::ShrAssign
            | InfixOp::Assign => (1, 0),
            InfixOp::Or => (2, 3),
            InfixOp::And => (4, 5),
            InfixOp::BitOr => (6, 7),
            InfixOp::Xor => (8, 9),
            InfixOp::BitAnd => (10, 11),
            InfixOp::Cmp(_) => (12, 13),
            InfixOp::Shl | InfixOp::Shr => (14, 15),
            // TokenTree::Punct(Punct::DotDot | Punct::DotDotEq) => Some((6, 7)),
            InfixOp::Add | InfixOp::Sub => (16, 17),
            InfixOp::Mul | InfixOp::Div | InfixOp::Mod => (18, 19),
        }
    }

    /// Convert from an assign operator to the normal form (`AddAssign` => `Add`)
    pub(crate) fn strip_assign(&self) -> Option<Self> {
        match self {
            Self::Cmp(_) => None,
            Self::Assign => None,
            Self::Or => None,
            Self::OrAssign => Some(Self::Or),
            Self::And => None,
            Self::AndAssign => Some(Self::And),
            Self::Add => None,
            Self::AddAssign => Some(Self::Add),
            Self::Sub => None,
            Self::SubAssign => Some(Self::Sub),
            Self::Mul => None,
            Self::MulAssign => Some(Self::Mul),
            Self::Div => None,
            Self::DivAssign => Some(Self::Div),
            Self::Mod => None,
            Self::ModAssign => Some(Self::Mod),
            Self::BitAnd => None,
            Self::BitAndAssign => Some(Self::BitAnd),
            Self::BitOr => None,
            Self::BitOrAssign => Some(Self::BitOr),
            Self::Xor => None,
            Self::XorAssign => Some(Self::Xor),
            Self::Shl => None,
            Self::ShlAssign => Some(Self::Shl),
            Self::Shr => None,
            Self::ShrAssign => Some(Self::Shr),
        }
    }
}

#[derive(Clone, Copy, Debug, derive_more::Display)]
pub enum PostfixOp {
    #[display("!")]
    AssertNotNull,
}

impl PostfixOp {
    fn bp(op: &TokenTree) -> Option<(u8, ())> {
        match op.inner {
            TokenTreeInner::Punct(Punct::Dot)
            | TokenTreeInner::Punct(Punct::Bang)
            | TokenTreeInner::Group {
                delim: GroupDelim::Paren | GroupDelim::Bracket,
                ..
            } => Some((20, ())),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, derive_more::Display)]
pub enum ObjectFieldKV {
    /// `{ foo }`
    #[display("{_0}")]
    Ident(Ident),
    /// `{ foo: 1 + 2 }`
    #[display("{_0}: {_1}")]
    IdentWithValue(Ident, Expr),
    /// `{ "foo": 1 + 2 }`
    #[display("{_0}: {_1}")]
    String(StringExpr, Expr),
    /// Used for setting object field programmatically
    #[display("{_0}: {}", _1.display())]
    StringValue(IStr, ValueRef),
}

impl Parse for ObjectFieldKV {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        let mut la = tokens.lookahead();
        let kvp = if la.peek(Identifier) {
            let ident = tokens.parse()?;
            let mut la = tokens.lookahead();

            if la.peek(Punct::Colon) {
                tokens.expect(Punct::Colon)?;
                Self::IdentWithValue(ident, tokens.parse()?)
            } else if la.peek(Punct::Comma) || la.eof("End of object literal") {
                Self::Ident(ident)
            } else {
                return la.error();
            }
        } else if la.peek(LitKind::String) {
            let key = tokens.parse()?;
            tokens.expect(Punct::Colon)?;

            Self::String(key, tokens.parse()?)
        } else {
            return la.error();
        };

        Ok(kvp)
    }
}

#[derive(Clone, Debug)]
pub struct ObjectField {
    pub condition: Option<Expr>,
    pub kv: ObjectFieldKV,
}

impl Display for ObjectField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(c) = &self.condition {
            write!(f, "if {} then {}", c, self.kv)
        } else {
            self.kv.fmt(f)
        }
    }
}

impl Parse for ObjectField {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        Ok(Self {
            condition: if tokens.next_if(Keyword::If).is_some() {
                let expr = tokens.parse()?;
                tokens.expect(Keyword::Then)?;
                Some(expr)
            } else {
                None
            },
            kv: tokens.parse()?,
        })
    }
}

#[derive(Clone, Debug, derive_more::Display)]
pub enum Ast {
    #[display("{_0}")]
    String(StringExpr),
    #[display("{_0}")]
    Lit(Lit),
    #[display("{_0}")]
    Block(Block),
    #[display("{_0}")]
    Variable(Ident),
    #[display("request")]
    Request,
    #[display("response")]
    Response,
    #[display("{}{}", op, operand)]
    PrefixOp {
        op: PrefixOp,
        op_span: Span,
        operand: Box<Expr>,
    },
    #[display("({} {} {})", operands.0, op, operands.1)]
    InfixOp {
        op: InfixOp,
        op_span: Span,
        operands: Box<(Expr, Expr)>,
    },
    #[display("{}{}", operand, op)]
    PostfixOp {
        op: PostfixOp,
        op_span: Span,
        operand: Box<Expr>,
    },
    #[display("let {} = {}", var, if let Some(value) = value { format!("Some({})", value) } else { "None".into() })]
    Declare {
        var: Ident,
        value: Option<Box<Expr>>,
    },
    #[display("{}.{}", value, field)]
    FieldAccess { value: Box<Expr>, field: Ident },
    #[display("{}.{}({})", value, method, DisplayVec(args))]
    MethodCall {
        value: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
    },
    #[display("{}[{}]", value, index)]
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
        question: Option<Span>,
    },
    #[display("{}({})", func, DisplayList(args))]
    FunctionCall {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    #[display("if {} {{ {} }}{}", condition, then, if let Some(elze) = elze { format!(" else {{ {} }}", elze) } else { "".into() })]
    If {
        condition: Box<Expr>,
        then: Box<Expr>,
        elze: Option<Box<Expr>>,
    },
    #[display("{}", DisplayVec(items))]
    ArrayLiteral { items: Vec<Expr> },
    #[display("{}", DisplayVec(fields))]
    ObjectLiteral {
        // ident: Ident,
        fields: Vec<ObjectField>,
    },
}

#[derive(Debug, Clone, derive_more::Display)]
#[display("{}", ast)]
pub struct Expr {
    pub(crate) ast: Ast,
    pub span: Span,
}

impl Expr {
    fn parse_lhs(tokens: &mut TokenStream, _min_bp: u8) -> Result<(Self, bool), ParseError> {
        let mut lhs_statement = false;
        let mut la = tokens.lookahead();
        let lhs = if la.peek(GroupDelim::Paren) {
            let mut tokens = tokens.expect_group(GroupDelim::Paren)?;
            if tokens.next_if(Punct::Arrow).is_some() {
                todo!("lambda expressions");
            }
            tokens.parse()?
        } else if la.peek(GroupDelim::Bracket) {
            let mut body = tokens.expect_group(GroupDelim::Bracket)?;
            let items = Self::parse_comma_sep_exprs(&mut body)?;
            Expr {
                span: body.span(),
                ast: Ast::ArrayLiteral { items },
            }
        } else if la.peek(GroupDelim::Brace) {
            let mut body = tokens.expect_group(GroupDelim::Brace)?;
            let fields = Self::parse_object_fields(&mut body)?;
            Expr {
                span: body.span(),
                ast: Ast::ObjectLiteral { fields },
            }
        } else if la.peek(Keyword::Let) {
            let _let = tokens.expect(Keyword::Let)?;
            let var = tokens.parse()?;
            let value = if tokens.next_if(Punct::Eq).is_some() {
                Some(Box::new(tokens.parse()?))
            } else {
                None
            };
            Expr {
                ast: Ast::Declare { var, value },
                span: _let.span,
            }
        } else if la.peek(Keyword::If) {
            lhs_statement = true;
            let _if = tokens.expect(Keyword::If)?;

            let condition = Self::parse(tokens)?;

            let mut la = tokens.lookahead();
            let (then, elze) = if la.peek(Keyword::Then) {
                let _then = tokens.expect(Keyword::Then)?;
                let then = tokens.parse()?;
                let elze = if tokens.next_if(Keyword::Else).is_some() {
                    Some(Box::new(tokens.parse()?))
                } else {
                    None
                };

                (then, elze)
            } else if la.peek(GroupDelim::Brace) {
                let block: Block = tokens.parse()?;
                let then = Expr {
                    span: block.span,
                    ast: Ast::Block(block),
                };

                let elze = if tokens.next_if(Keyword::Else).is_some() {
                    let block: Block = tokens.parse()?;
                    let elze = Expr {
                        span: block.span,
                        ast: Ast::Block(block),
                    };
                    Some(Box::new(elze))
                } else {
                    None
                };

                (then, elze)
            } else {
                return la.error();
            };

            Expr {
                ast: Ast::If {
                    condition: Box::new(condition),
                    then: Box::new(then),
                    elze,
                },
                span: _if.span,
            }

            // TODO: while, for, break, continue
        } else if la.peek(Keyword::True) {
            let t = tokens.expect(Keyword::True)?;
            Expr {
                ast: Ast::Lit(Lit::Bool(true)),
                span: t.span,
            }
        } else if la.peek(Keyword::False) {
            let t = tokens.expect(Keyword::False)?;
            Expr {
                ast: Ast::Lit(Lit::Bool(false)),
                span: t.span,
            }
        } else if la.peek(Keyword::Null) {
            let t = tokens.expect(Keyword::Null)?;
            Expr {
                ast: Ast::Lit(Lit::Null),
                span: t.span,
            }
        } else if la.peek(LitKind::Int) {
            let t = tokens.expect(LitKind::Int)?;
            let TokenTreeInner::Literal(lex::Lit::Int(n)) = t.inner else {
                unreachable!()
            };
            Expr {
                ast: Ast::Lit(Lit::Int(n)),
                span: t.span,
            }
        } else if la.peek(LitKind::Float) {
            let t = tokens.expect(LitKind::Float)?;
            let TokenTreeInner::Literal(lex::Lit::Float(n)) = t.inner else {
                unreachable!()
            };
            Expr {
                ast: Ast::Lit(Lit::Float(n)),
                span: t.span,
            }
        } else if la.peek(LitKind::String) {
            let strexpr: StringExpr = tokens.parse()?;
            Expr {
                span: strexpr.span,
                ast: Ast::String(strexpr),
            }
        } else if la.peek(Identifier) {
            let span = la.span().expect("peek is true");
            let ident = tokens.parse()?;
            Expr {
                ast: Ast::Variable(ident),
                span,
            }
        } else if la.peek(Keyword::Request) {
            let tt = tokens.expect(Keyword::Request)?;
            Expr {
                ast: Ast::Request,
                span: tt.span,
            }
        } else if la.peek(Keyword::Response) {
            let tt = tokens.expect(Keyword::Response)?;
            Expr {
                ast: Ast::Response,
                span: tt.span,
            }
        } else if la.peek(Punct::Minus) || la.peek(Punct::Bang) {
            // prefix operators
            let tt = tokens.expect_any()?;
            let op = PrefixOp::from_tt(&tt).expect("Checked in if");
            let ((), r_bp) = op.bp();
            let rhs = Self::parse_bp(tokens, r_bp)?;
            Expr {
                span: tt.span + rhs.span,
                ast: Ast::PrefixOp {
                    op,
                    op_span: tt.span,
                    operand: Box::new(rhs),
                },
            }
        } else {
            return la.error();
        };
        Ok((lhs, lhs_statement))
    }

    fn parse_postfix(tokens: &mut TokenStream, lhs: Self) -> Result<Self, ParseError> {
        let lhs_span = lhs.span;
        let op = tokens.expect_any()?;
        let lhs = match op.inner {
            TokenTreeInner::Group {
                delim: GroupDelim::Paren,
                mut tokens,
            } => {
                let span = lhs.span + tokens.span();
                Expr {
                    ast: Ast::FunctionCall {
                        span: lhs.span + op.span,
                        func: Box::new(lhs),
                        args: Self::parse_comma_sep_exprs(&mut tokens)?,
                    },
                    span,
                }
            }
            TokenTreeInner::Group {
                delim: GroupDelim::Bracket,
                tokens: mut inner_tokens,
            } => {
                let span = lhs.span + tokens.span();
                Expr {
                    ast: Ast::Index {
                        value: Box::new(lhs),
                        index: Box::new(inner_tokens.parse()?),
                        question: tokens.next_if(Punct::Question).map(|t| t.span),
                    },
                    span: lhs_span + span,
                }
            }
            TokenTreeInner::Punct(Punct::Bang) => Expr {
                ast: Ast::PostfixOp {
                    op: PostfixOp::AssertNotNull,
                    op_span: op.span,
                    operand: Box::new(lhs),
                },
                span: lhs_span + op.span,
            },
            TokenTreeInner::Punct(Punct::Dot) => {
                let span = tokens.peek().map(|t| t.span);
                let ident = tokens.parse()?;
                let span = span.unwrap();
                if tokens.peek_kind(GroupDelim::Paren).is_some() {
                    let mut tokens = tokens.expect_group(GroupDelim::Paren)?;
                    let args = Self::parse_comma_sep_exprs(&mut tokens)?;
                    Expr {
                        ast: Ast::MethodCall {
                            value: Box::new(lhs),
                            method: ident,
                            args,
                        },
                        span: lhs_span + op.span + span + tokens.span(),
                    }
                } else {
                    Expr {
                        ast: Ast::FieldAccess {
                            value: Box::new(lhs),
                            field: ident,
                        },
                        span: lhs_span + op.span + span,
                    }
                }
            }
            t => unreachable!("t = {:?}", t),
        };

        Ok(lhs)
    }

    fn parse_comma_sep_exprs(tokens: &mut TokenStream) -> Result<Vec<Expr>, ParseError> {
        let mut exprs = Vec::new();
        while !tokens.is_empty() {
            exprs.push(tokens.parse()?);
            let mut la = tokens.lookahead();

            if la.peek(Punct::Comma) {
                tokens.expect(Punct::Comma)?;
                continue;
            } else if la.eof("End of arguments") {
                break;
            } else {
                return la.error();
            }
        }
        Ok(exprs)
    }

    fn parse_bp(tokens: &mut TokenStream, min_bp: u8) -> Result<Self, ParseError> {
        let (mut lhs, lhs_statement) = Self::parse_lhs(tokens, min_bp)?;

        loop {
            let mut la = tokens.lookahead();
            if la.eof("End of expression")
                || la.peek(Punct::Comma)
                || la.peek(Punct::Semicolon)
                || la.peek(Keyword::Then)
                || la.peek(Keyword::Else)
                || la.peek(Keyword::Before)
                || la.peek(Keyword::After)
            {
                break;
            } else if la.peek(GroupDelim::Paren)
                || la.peek(GroupDelim::Bracket)
                || la.peek(GroupDelim::Brace)
                || la.peek(Operator)
            {
                // fallthrough
            } else if lhs_statement {
                break;
            } else {
                return la.error_expected([&*Punct::Semicolon.to_string(), "Operator"]);
            }

            if let Some((l_bp, ())) = PostfixOp::bp(tokens.peek().expect("checked in la")) {
                if l_bp < min_bp {
                    break;
                }
                lhs = Self::parse_postfix(tokens, lhs)?;

                continue;
            }

            if let Some(op) = InfixOp::from_tt(tokens.peek().expect("checked in la")) {
                let (l_bp, r_bp) = op.bp();
                if l_bp < min_bp {
                    break;
                }
                let tt = tokens.expect_any()?;

                let rhs = Self::parse_bp(tokens, r_bp)?;
                lhs = Expr {
                    span: lhs.span + rhs.span,
                    ast: Ast::InfixOp {
                        op,
                        op_span: tt.span,
                        operands: Box::new((lhs, rhs)),
                    },
                };
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn parse_object_fields(tokens: &mut TokenStream) -> Result<Vec<ObjectField>, ParseError> {
        let mut fields = Vec::new();

        while !tokens.is_empty() {
            fields.push(tokens.parse()?);

            if tokens.next_if(Punct::Comma).is_none() {
                break;
            }
        }

        Ok(fields)
    }
}

impl Parse for Expr {
    fn parse(tokens: &mut TokenStream) -> Result<Self, ParseError> {
        Self::parse_bp(tokens, 0)
    }
}
