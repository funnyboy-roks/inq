use std::{
    any::TypeId,
    cell::RefCell,
    collections::{HashMap, hash_map::Entry},
    ops::Not,
    rc::Rc,
    sync::Arc,
};

pub(crate) mod registry;
pub(crate) mod value;

use miette::Diagnostic;
use thiserror::Error;

use crate::lang::{
    eval::{
        registry::{AnyRegistry, BinOp, DynMethod, Field, Indexer, Registry, UnaryOp, VarArgs},
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Float, Int, Null, Object},
        },
    },
    expr::{Ast, Expr},
    parse::{Ident, StringExpr},
    string::IStr,
    util::{DisplayVec, Span},
};

use super::expr::{InfixOp, Lit, ObjectField};

#[derive(Debug, Clone, Error, Diagnostic)]
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
    #[error("Invalid argments, expected {}, got {}", expected, got)]
    InvalidArgs {
        #[label = "this call"]
        span: Span,
        got: DisplayVec<String>,
        expected: DisplayVec<String>,
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
}

pub type EvalResult<T> = Result<T, EvalError>;

#[derive(Default, Debug)]
struct TypeRegistry(HashMap<TypeId, AnyRegistry>);
impl TypeRegistry {
    fn get(&self, value: &ValueRef, span: Span) -> EvalResult<&AnyRegistry> {
        let tid = value.type_id();
        self.0.get(&tid).ok_or_else(|| EvalError::UnknownType {
            ty: value.type_name_of().into(),
            span,
        })
    }

    fn get_field(&self, value: &ValueRef, span: Span, field: &Ident) -> EvalResult<&Field> {
        let reg = self.get(value, span)?;
        reg.get_field(&field.inner)
            .ok_or_else(|| EvalError::UnknownField {
                ty: value.type_name_of().into(),
                field: field.clone(),
            })
    }

    fn get_method(
        &self,
        value: &ValueRef,
        span: Span,
        method: &Ident,
    ) -> EvalResult<&dyn DynMethod> {
        let reg = self.get(value, span)?;
        reg.get_method(&method.inner)
            .ok_or_else(|| EvalError::UnknownMethod {
                ty: value.type_name_of().into(),
                method: method.clone(),
            })
    }

    fn get_index(&self, value: &ValueRef, index: &ValueRef, span: Span) -> EvalResult<&Indexer> {
        let reg = self.get(value, span)?;
        reg.get_index(index.type_id())
            .ok_or_else(|| EvalError::InvalidIndex {
                ty: value.type_name_of().into(),
                index: index.type_name_of().into(),
                span,
            })
    }
}

#[derive(Default, Debug)]
pub struct Context {
    variables: RefCell<HashMap<IStr, ValueRef>>,
    types: TypeRegistry,
}

impl Context {
    pub fn new() -> Self {
        let mut this = Self {
            variables: Default::default(),
            types: Default::default(),
        };

        this.register_defaults();

        this
    }

    pub fn register_defaults(&mut self) {
        self.register_type::<String>();
        self.register_type::<Int>();
        self.register_type::<Float>();
        self.register_type::<Null>();
        self.register_type::<bool>();
        self.register_type::<Array>();
        self.register_type::<Object>();
    }

    pub fn register_type<V: Value>(&mut self) {
        match self.types.0.entry(TypeId::of::<V>()) {
            Entry::Occupied(_) => {}
            Entry::Vacant(e) => {
                let mut reg = Registry::new();
                V::register(&mut reg);
                let (_, reg) = reg.erase();
                e.insert_entry(reg);
            }
        }
    }

    pub(crate) fn get_variable(&self, name: &str) -> Option<ValueRef> {
        self.variables.borrow().get(name).cloned()
    }

    pub(crate) fn set_variable<V>(&mut self, name: impl Into<IStr>, value: V) -> Option<ValueRef>
    where
        V: Value,
    {
        self.register_type::<V>();
        self.variables
            .borrow_mut()
            .insert(name.into(), value.into())
    }

    pub(crate) fn set_variable_by_ref(
        &self,
        name: impl Into<IStr>,
        value: ValueRef,
    ) -> Option<ValueRef> {
        self.variables.borrow_mut().insert(name.into(), value)
    }
}

/// Evaluation
impl Context {
    pub(crate) fn eval(self: &Rc<Self>, expr: Expr) -> EvalResult<ValueRef> {
        match expr.ast {
            Ast::String(string_expr) => Ok(self.eval_string(string_expr)?.into()),
            Ast::Lit(lit) => match lit {
                Lit::Int(n) => Ok(ValueRef::new(n as i64)),
                Lit::Float(f) => Ok(ValueRef::new(f)),
                Lit::Bool(b) => Ok(ValueRef::new(b)),
                Lit::Null => Ok(ValueRef::new(Null)),
            },
            Ast::Block(block) => {
                let mut last = ValueRef::null();
                for e in block.exprs {
                    last = self.eval(e)?;
                }
                if block.ret {
                    Ok(last)
                } else {
                    Ok(ValueRef::null())
                }
            }
            Ast::Variable(ident) => self
                .get_variable(&ident.inner)
                .ok_or(EvalError::UndefinedVariable { ident }),
            Ast::Request => todo!(),
            Ast::Response => todo!(),
            Ast::PrefixOp {
                op,
                op_span,
                operand,
            } => {
                let operand = self.eval(*operand)?;
                let op = match op {
                    crate::lang::expr::PrefixOp::Neg => UnaryOp::Prefix(registry::PrefixOp::Neg),
                    crate::lang::expr::PrefixOp::Not => {
                        return Ok(operand.borrow().truthy().not().into());
                    }
                };
                let Some(unary_op) = self.types.get(&operand, op_span)?.get_unary_op(op) else {
                    return Err(EvalError::InvalidUnaryOp {
                        op,
                        span: op_span,
                        operand: operand.type_name_of().into(),
                    });
                };
                unary_op.apply(CallContext {
                    span: op_span,
                    inner: (),
                    self_ref: operand,
                })
            }
            Ast::InfixOp {
                op,
                op_span,
                operands,
            } => {
                let (lhs, rhs) = *operands;
                if op == InfixOp::Assign {
                    return match lhs.ast {
                        Ast::Variable(var) => {
                            let rhs = self.eval(rhs)?;
                            if self.set_variable_by_ref(var.inner.clone(), rhs).is_none() {
                                return Err(EvalError::UndefinedVariable { ident: var });
                            } else {
                                Ok(ValueRef::null())
                            }
                        }
                        Ast::FieldAccess { value, field } => {
                            let value_span = value.span;
                            let obj = self.eval(*value)?;
                            let value = self.eval(rhs)?;

                            let setter = self
                                .types
                                .get_field(&obj, value_span, &field)?
                                .setter
                                .as_ref()
                                .ok_or_else(|| EvalError::ReadonlyField {
                                    ty: value.type_name_of().into(),
                                    field: field.clone(),
                                })?;

                            setter.set(
                                value,
                                CallContext {
                                    span: field.span,
                                    inner: (),
                                    self_ref: obj,
                                },
                            )?;

                            Ok(ValueRef::null())
                        }
                        Ast::Index { value, index } => {
                            let rhs = self.eval(rhs)?;
                            let span = index.span;
                            let value = self.eval(*value)?;
                            let index = self.eval(*index)?;
                            let setter = self
                                .types
                                .get_index(&value, &index, span)?
                                .setter
                                .as_ref()
                                .ok_or_else(|| EvalError::ReadonlyIndex {
                                    ty: value.type_name_of().into(),
                                    index: index.type_name_of().into(),
                                    span,
                                })?;
                            setter.set(
                                index,
                                rhs,
                                CallContext {
                                    span,
                                    inner: (),
                                    self_ref: value.clone(),
                                },
                            )?;

                            Ok(ValueRef::null())
                        }
                        _ => Err(EvalError::InvalidAssignment {
                            span: op_span,
                            lhs_span: lhs.span,
                        }),
                    };
                }

                let op = match op {
                    InfixOp::Assign => unreachable!("handled above"),
                    InfixOp::Or => {
                        let lhs = self.eval(lhs)?;
                        if lhs.borrow().truthy() {
                            return Ok(lhs);
                        } else {
                            return self.eval(rhs);
                        }
                    }
                    InfixOp::And => {
                        let lhs = self.eval(lhs)?;
                        if lhs.borrow().truthy() {
                            return self.eval(rhs);
                        } else {
                            return Ok(lhs);
                        }
                    }
                    InfixOp::Equality => todo!(),
                    InfixOp::Add => BinOp::Add,
                    InfixOp::Sub => BinOp::Sub,
                    InfixOp::Mul => BinOp::Mul,
                    InfixOp::Div => BinOp::Div,
                };

                let lhs_span = lhs.span;
                let lhs = self.eval(lhs)?;
                let rhs = self.eval(rhs)?;

                let registry = self.types.get(&lhs, lhs_span)?;

                let Some(binop) = registry.get_bin_op(op, rhs.type_id()) else {
                    return Err(EvalError::InvalidBinOp {
                        op,
                        span: op_span,
                        lhs: lhs.type_name_of().into(),
                        rhs: rhs.type_name_of().into(),
                    });
                };

                binop.apply(
                    lhs.clone(),
                    rhs,
                    CallContext {
                        span: op_span,
                        inner: (),
                        self_ref: lhs,
                    },
                )
            }
            Ast::PostfixOp {
                op,
                op_span,
                operand,
            } => {
                let span = operand.span;
                let operand = self.eval(*operand)?;
                match op {
                    super::expr::PostfixOp::AssertNotNull => {
                        if operand.borrow().is::<Null>() {
                            Err(EvalError::NotNullAssertion { span })
                        } else {
                            Ok(operand)
                        }
                    }
                }
            }
            Ast::Declare { var, value } => {
                let value = if let Some(value) = value {
                    self.eval(*value)?
                } else {
                    ValueRef::null()
                };
                self.set_variable_by_ref(var.inner.clone(), value);
                Ok(ValueRef::null())
            }
            Ast::FieldAccess { value, field } => {
                let value_span = value.span;
                let value = self.eval(*value)?;
                let span = field.span;
                let getter = &self.types.get_field(&value, value_span, &field)?.getter;
                getter.get(CallContext {
                    span,
                    inner: (),
                    self_ref: value,
                })
            }
            Ast::MethodCall {
                value,
                method,
                args,
            } => {
                let value_span = value.span;
                let value = self.eval(*value)?;

                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    eval_args.push(self.eval(a)?);
                }

                let span = method.span;
                let func = &self.types.get_method(&value, value_span, &method)?;
                func.call(
                    VarArgs::new(eval_args),
                    CallContext {
                        span,
                        inner: (),
                        self_ref: value,
                    },
                )
            }
            Ast::Index { value, index } => {
                let span = index.span;
                let value = self.eval(*value)?;
                let index = self.eval(*index)?;
                let idx = self.types.get_index(&value, &index, span)?;
                idx.getter.get(
                    index,
                    CallContext {
                        span,
                        inner: (),
                        self_ref: value.clone(),
                    },
                )
            }
            Ast::FunctionCall { func, args, span } => {
                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    eval_args.push(self.eval(a)?);
                }
                let func = self.eval(*func)?;
                let reg = self.types.get(&func, span)?;
                if let Some(call) = &reg.call {
                    call.call(
                        VarArgs::new(eval_args),
                        CallContext {
                            span,
                            inner: (),
                            self_ref: func.clone(),
                        },
                    )
                } else {
                    Err(EvalError::NotCallable {
                        ty: func.type_name_of().into(),
                        span,
                    })
                }
            }
            Ast::If {
                condition,
                then,
                elze,
            } => {
                let condition = self.eval(*condition)?;
                if condition.borrow().truthy() {
                    self.eval(*then)
                } else {
                    if let Some(elze) = elze {
                        self.eval(*elze)
                    } else {
                        Ok(ValueRef::null())
                    }
                }
            }
            Ast::ArrayLiteral { items } => Ok(items
                .into_iter()
                .map(|i| self.eval(i))
                .collect::<Result<Vec<_>, _>>()?
                .into()),
            Ast::ObjectLiteral { fields } => {
                let mut inner = HashMap::<IStr, ValueRef>::new();
                for f in fields {
                    let (k, v) = match f {
                        ObjectField::Ident(ident) => {
                            let s = ident.inner.clone();
                            let val = self
                                .get_variable(&ident.inner)
                                .ok_or(EvalError::UndefinedVariable { ident })?;
                            (s, val)
                        }
                        ObjectField::IdentWithValue(ident, expr) => (ident.inner, self.eval(expr)?),
                        ObjectField::String(string_expr, expr) => {
                            (self.eval_string(string_expr)?.into(), self.eval(expr)?)
                        }
                    };
                    inner.insert(k, v);
                }
                Ok(Object(inner).into())
            }
        }
    }

    fn eval_string(self: &Rc<Self>, string: StringExpr) -> EvalResult<String> {
        let mut out = String::new();
        let mut last = 0;
        for e in string.interpolations {
            out.push_str(&string.value[last..e.index]);
            self.eval(e.expr)?.borrow().to_string(&mut out);
            last = e.index;
        }
        out.push_str(&string.value[last..]);
        Ok(out)
    }
}
