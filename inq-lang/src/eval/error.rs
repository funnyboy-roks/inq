use miette::Diagnostic;
use thiserror::Error;

use crate::{
    Ident, Span,
    eval::registry::{BinOp, UnaryOp},
    util::DisplayVec,
};

#[derive(Debug, Error, Diagnostic)]
pub enum EvalError {
    #[error("Unknown field '{}' on type {}", field, ty)]
    UnknownField {
        ty: String,
        #[label]
        field: Ident,
    },
    #[error("Cannot index into type {} with type {}", ty, index_ty)]
    CannotIndex {
        ty: String,
        index_ty: String,
        #[label]
        span: Span,
    },
    #[error("Unknown method '{}' on type {}", method, ty)]
    UnknownMethod {
        ty: String,
        #[label]
        method: Ident,
    },
    #[error("Cannot call type {}", ty)]
    NotCallable {
        ty: String,
        #[label]
        span: Span,
    },
    #[error("Cannot perfom '{}' operation on {}", op, lhs)]
    InvalidOperation {
        op: String,
        #[label]
        span: Span,
        lhs: String,
    },
    #[error("Cannot perfom '{}' operation on {} and {}", op, lhs, rhs)]
    InvalidBinOp {
        op: BinOp,
        #[label]
        span: Span,
        lhs: String,
        rhs: String,
    },
    #[error("Cannot perfom '{}' operation on {}", op, operand)]
    InvalidUnaryOp {
        op: UnaryOp,
        #[label]
        span: Span,
        operand: String,
    },
    #[error("Undefined Variable: {}", ident)]
    UndefinedVariable {
        #[label]
        ident: Ident,
    },
    #[error(
        "Invalid argments, expected {}, got {}",
        DisplayVec(expected),
        DisplayVec(got)
    )]
    InvalidArgs {
        #[label = "this call"]
        span: Span,
        got: Vec<String>,
        expected: Vec<String>,
    },
    #[error("Invalid left-hand side of assignment operator.  ")]
    InvalidAssignment {
        #[label(primary, "This assignment")]
        span: Span,
        #[label = "Must be variable, field, or index."]
        lhs_span: Span,
    },
    #[error("This field is read-only")]
    ReadonlyField {
        ty: String,
        #[label = "this field"]
        field: Ident,
    },
    #[error("Index into type {} with type {} is read-only", ty, index)]
    ReadonlyIndex {
        ty: String,
        index: String,
        #[label = "here"]
        span: Span,
    },
    #[error("Unregistered type: {}", ty)]
    UnknownType {
        ty: String,
        #[label = "found here"]
        span: Span,
    },
    #[error("{}", message)]
    Custom {
        message: String,
        #[label = "here"]
        span: Span,
    },
    #[error("Cannot index into type {} with type {}", ty, index)]
    InvalidIndex {
        ty: String,
        index: String,
        #[label = "here"]
        span: Span,
    },
    #[error("Object does not have index {}", index)]
    IndexNotFounc {
        index: String,
        #[label = "here"]
        span: Span,
    },
    #[error("Non-null assertion failed")]
    NotNullAssertion {
        #[label = "this expression"]
        span: Span,
    },
    #[error("Unable to compare types {} and {}", lhs, rhs)]
    InvalidCmp {
        lhs: String,
        rhs: String,
        #[label = "This comparison"]
        span: Span,
    },
    #[error("Expected {} got {}", expected, actual)]
    InvalidType {
        expected: String,
        actual: String,
        #[label = "here"]
        span: Span,
    },
    #[error("`request` may only be used in the `before` block")]
    RequestInBadPosition {
        #[label = "here"]
        span: Span,
    },
    #[error("`response` may only be used in the `after` block")]
    ResponseInBadPosition {
        #[label = "here"]
        span: Span,
    },
    #[error("Attempt to divide by zero")]
    Div0 {
        #[label = "here"]
        span: Span,
    },
    #[error("'?' not allowed on left side of assignment")]
    InvalidQuestion {
        #[label = "here"]
        span: Span,
    },
    #[error("Missing index {} on type {}", index, value)]
    MissingIndex {
        value: String,
        index: String,
        #[label = "here"]
        span: Span,
    },
    #[error("{}", _0)]
    #[diagnostic(transparent)]
    Transparent(miette::Error),
}

impl From<miette::Report> for EvalError {
    fn from(value: miette::Report) -> Self {
        Self::Transparent(value)
    }
}

impl EvalError {
    pub fn custom(span: impl Into<Span>, message: impl Into<String>) -> Self {
        Self::Custom {
            message: message.into(),
            span: span.into(),
        }
    }
}

pub type EvalResult<T> = Result<T, EvalError>;
