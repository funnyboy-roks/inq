use crate::{
    Span,
    lex::{self, GroupDelim, Keyword, LitKind, Punct, TokenKind, TokenStream, TokenTree},
    parse::{Block, Ident, Parse, ParseError, StringExpr},
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

#[derive(Clone, Copy, Debug)]
pub enum Lit {
    Int(u64),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Clone, Copy, Debug)]
pub enum PrefixOp {
    Neg,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    /// `<`
    Lt,
    /// `<=`
    Lte,
    /// `>`
    Gt,
    /// `>=`
    Gte,
    /// `==`
    Eq,
    /// `!=`
    NotEq,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfixOp {
    Assign,
    Or,
    And,
    Cmp(CmpOp),
    Add,
    Sub,
    Mul,
    Div,
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
            TokenTreeInner::Punct(Punct::Minus) => Some(Self::Sub),
            TokenTreeInner::Punct(Punct::Star) => Some(Self::Mul),
            TokenTreeInner::Punct(Punct::Slash) => Some(Self::Div),
            TokenTreeInner::Punct(Punct::PipePipe) => Some(Self::Or),
            TokenTreeInner::Punct(Punct::AndAnd) => Some(Self::And),
            _ => None,
        }
    }

    fn bp(self) -> (u8, u8) {
        match self {
            InfixOp::Assign => (0, 1),
            InfixOp::Or => (2, 3),
            InfixOp::And => (4, 5),
            InfixOp::Cmp(_) => (6, 7),
            // TokenTree::Punct(Punct::EqEq | Punct::Lt | Punct::Lte | Punct::Gte | Punct::Gt) => {
            //     Some((4, 5))
            // }
            // TokenTree::Punct(Punct::DotDot | Punct::DotDotEq) => Some((6, 7)),
            InfixOp::Add | InfixOp::Sub => (8, 9),
            InfixOp::Mul | InfixOp::Div => (10, 11),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PostfixOp {
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

#[derive(Clone, Debug)]
pub(crate) enum ObjectField {
    /// `{ foo }`
    Ident(Ident),
    /// `{ foo: 1 + 2 }`
    IdentWithValue(Ident, Expr),
    /// `{ "foo": 1 + 2 }`
    String(StringExpr, Expr),
}

#[derive(Clone, Debug)]
pub enum Ast {
    String(StringExpr),
    Lit(Lit),
    Block(Block),
    Variable(Ident),
    Request,
    Response,
    PrefixOp {
        op: PrefixOp,
        op_span: Span,
        operand: Box<Expr>,
    },
    InfixOp {
        op: InfixOp,
        op_span: Span,
        operands: Box<(Expr, Expr)>,
    },
    PostfixOp {
        op: PostfixOp,
        op_span: Span,
        operand: Box<Expr>,
    },
    Declare {
        var: Ident,
        value: Option<Box<Expr>>,
    },
    FieldAccess {
        value: Box<Expr>,
        field: Ident,
    },
    MethodCall {
        value: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
    },
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    FunctionCall {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    If {
        condition: Box<Expr>,
        then: Box<Expr>,
        elze: Option<Box<Expr>>,
    },
    ArrayLiteral {
        items: Vec<Expr>,
    },
    ObjectLiteral {
        // ident: Ident,
        fields: Vec<ObjectField>,
    },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub(crate) ast: Ast,
    pub(crate) span: Span,
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
                mut tokens,
            } => {
                let span = lhs.span + tokens.span();
                Expr {
                    ast: Ast::Index {
                        value: Box::new(lhs),
                        index: Box::new(tokens.parse()?),
                    },
                    span,
                }
            }
            TokenTreeInner::Punct(Punct::Bang) => Expr {
                ast: Ast::PostfixOp {
                    op: PostfixOp::AssertNotNull,
                    op_span: op.span,
                    operand: Box::new(lhs),
                },
                span: op.span,
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
                        span: op.span + span + tokens.span(),
                    }
                } else {
                    Expr {
                        ast: Ast::FieldAccess {
                            value: Box::new(lhs),
                            field: ident,
                        },
                        span: op.span + span,
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
                return la.error_expected(["Operator"]);
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
            let mut la = tokens.lookahead();
            let field = if la.peek(Identifier) {
                let ident = tokens.parse()?;
                let mut la = tokens.lookahead();

                if la.peek(Punct::Colon) {
                    tokens.expect(Punct::Colon)?;
                    ObjectField::IdentWithValue(ident, tokens.parse()?)
                } else if la.peek(Punct::Comma) || la.eof("End of object literal") {
                    ObjectField::Ident(ident)
                } else {
                    return la.error();
                }
            } else if la.peek(LitKind::String) {
                let key = tokens.parse()?;
                tokens.expect(Punct::Colon)?;

                ObjectField::String(key, tokens.parse()?)
            } else {
                return la.error();
            };

            fields.push(field);

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
