use std::{
    any::TypeId,
    cell::{OnceCell, Ref, RefCell},
    cmp::Ordering,
    collections::{BTreeMap, HashMap, hash_map::Entry},
    ops::Not,
    rc::Rc,
};

pub(crate) mod lazy;
pub mod registry;
pub mod value;

use miette::Diagnostic;
use thiserror::Error;

use crate::{
    IStr, Span,
    eval::{
        lazy::LazyValueRef,
        registry::{AnyRegistry, BinOp, DynMethod, Field, Indexer, Registry, UnaryOp, VarArgs},
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Float, Int, Null, Object},
        },
    },
    expr::{Ast, CmpOp, Expr},
    parse::{Ident, StringExpr},
    util::DisplayVec,
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
    #[error("Unable to compare types {} and {}", lhs, rhs)]
    InvalidCmp {
        lhs: String,
        rhs: String,
        #[label = "This comparison"]
        span: Span,
    },
}

pub type EvalResult<T> = Result<T, EvalError>;

#[derive(Default, Debug)]
struct TypeRegistry(HashMap<TypeId, Rc<AnyRegistry>>);
impl TypeRegistry {
    fn get(&self, value: &ValueRef, span: Span) -> EvalResult<Rc<AnyRegistry>> {
        let tid = value.type_id();
        self.0
            .get(&tid)
            .cloned()
            .ok_or_else(|| EvalError::UnknownType {
                ty: value.type_name_of().into(),
                span,
            })
    }
}

#[derive(Default, Debug)]
pub struct Engine {
    types: RefCell<TypeRegistry>,
    global: OnceCell<Rc<Scope>>,
}

impl Engine {
    pub fn new() -> Rc<Self> {
        let this = Self {
            types: Default::default(),
            global: OnceCell::new(),
        };
        this.register_defaults();
        let this = Rc::new(this);
        this.global.get_or_init({
            let this = this.clone();
            move || Scope::new(this).into()
        });
        this
    }

    fn register_defaults(&self) {
        self.register_type::<IStr>();
        self.register_type::<Int>();
        self.register_type::<Float>();
        self.register_type::<Null>();
        self.register_type::<bool>();
        self.register_type::<Array>();
        self.register_type::<Object>();
    }

    pub fn register_type<V: Value>(&self) {
        match self.types.borrow_mut().0.entry(TypeId::of::<V>()) {
            Entry::Occupied(_) => {}
            Entry::Vacant(e) => {
                let mut reg = Registry::new();
                V::register(&mut reg);
                let (_, reg) = reg.erase();
                e.insert_entry(reg.into());
            }
        }
    }

    pub fn global(self: &Rc<Self>) -> Rc<Scope> {
        self.global
            .get_or_init({
                let this = self.clone();
                move || Rc::new(Scope::new(this))
            })
            .clone()
    }

    fn types(&self) -> Ref<'_, TypeRegistry> {
        self.types.borrow()
    }

    fn get_type(&self, value: &ValueRef, span: Span) -> EvalResult<Rc<AnyRegistry>> {
        let types = self.types();
        types.get(value, span)
    }

    fn get_field(&self, value: &ValueRef, span: Span, field: &Ident) -> EvalResult<Field> {
        let types = self.types();
        let reg = types.get(value, span)?;
        reg.get_field(&field.inner)
            .cloned()
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
    ) -> EvalResult<Rc<dyn DynMethod>> {
        let types = self.types();
        let reg = types.get(value, span)?;
        reg.get_method(&method.inner)
            .ok_or_else(|| EvalError::UnknownMethod {
                ty: value.type_name_of().into(),
                method: method.clone(),
            })
    }

    fn get_index(&self, value: &ValueRef, index: &ValueRef, span: Span) -> EvalResult<Indexer> {
        let types = self.types();
        let reg = types.get(value, span)?;
        reg.get_index(index.type_id())
            .ok_or_else(|| EvalError::InvalidIndex {
                ty: value.type_name_of().into(),
                index: index.type_name_of().into(),
                span,
            })
    }
}

#[derive(Default, derive_more::Debug, Clone)]
pub struct Scope {
    parent: Option<Rc<Scope>>,
    variables: RefCell<HashMap<IStr, LazyValueRef>>,
    #[debug("..")]
    engine: Rc<Engine>,
}

impl Scope {
    fn new(engine: Rc<Engine>) -> Self {
        Self {
            parent: None,
            variables: Default::default(),
            engine,
        }
    }

    /// Take a snapshot of this scope such that modifying `self` does not modify the returned
    /// snapshot
    pub(crate) fn snapshot(&self) -> Self {
        let mut snapshot = Self::clone(self);
        snapshot.parent = snapshot.parent.map(|s| Rc::new(s.snapshot()));
        snapshot
    }

    pub(crate) fn child(self: &Rc<Self>) -> Rc<Self> {
        let mut this = Self::new(self.engine.clone());
        this.parent = Some(self.clone());
        this.into()
    }

    pub(crate) fn get_variable(&self, name: &str) -> Option<EvalResult<ValueRef>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Some(var.get())
        } else if let Some(parent) = &self.parent {
            parent.get_variable(name)
        } else {
            None
        }
    }

    pub(crate) fn expect_variable(&self, ident: &Ident) -> EvalResult<ValueRef> {
        self.get_variable(ident.as_ref())
            .ok_or(EvalError::UndefinedVariable {
                ident: ident.clone(),
            })?
    }

    /// Returns true if the variable already existed in the current scope
    pub fn set_variable<V>(&self, name: impl Into<IStr>, value: V, declare: bool) -> bool
    where
        V: Value,
    {
        self.engine.register_type::<V>();
        self.set_variable_by_ref(name, ValueRef::new(value), declare)
    }

    /// Returns true if the variable already existed
    pub(crate) fn set_variable_by_ref(
        &self,
        name: impl Into<IStr>,
        value: impl Into<LazyValueRef>,
        declare: bool,
    ) -> bool {
        let name = name.into();
        if declare {
            self.variables
                .borrow_mut()
                .insert(name, value.into())
                .is_some()
        } else {
            let mut vars = self.variables.borrow_mut();
            match vars.entry(name.clone()) {
                Entry::Occupied(mut e) => {
                    e.insert(value.into());
                    true
                }
                Entry::Vacant(_) if let Some(parent) = &self.parent => {
                    parent.set_variable_by_ref(name, value, declare)
                }
                Entry::Vacant(_) => false,
            }
        }
    }
}

/// Evaluation
impl Scope {
    pub fn eval(self: &Rc<Self>, expr: Expr) -> EvalResult<ValueRef> {
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
                    last = self.child().eval(e)?;
                }
                if block.ret {
                    Ok(last)
                } else {
                    Ok(ValueRef::null())
                }
            }
            Ast::Variable(ident) => self.expect_variable(&ident),
            Ast::Request => todo!(),
            Ast::Response => todo!(),
            Ast::PrefixOp {
                op,
                op_span,
                operand,
            } => {
                let operand = self.eval(*operand)?;
                let op = match op {
                    crate::expr::PrefixOp::Neg => UnaryOp::Prefix(registry::PrefixOp::Neg),
                    crate::expr::PrefixOp::Not => {
                        return Ok(operand.borrow().truthy().not().into());
                    }
                };
                let Some(unary_op) = self.engine.types().get(&operand, op_span)?.get_unary_op(op)
                else {
                    return Err(EvalError::InvalidUnaryOp {
                        op,
                        span: op_span,
                        operand: operand.type_name_of().into(),
                    });
                };
                unary_op.apply(CallContext {
                    span: op_span,
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
                            if self.set_variable_by_ref(var.inner.clone(), rhs, false) {
                                Ok(ValueRef::null())
                            } else {
                                return Err(EvalError::UndefinedVariable { ident: var });
                            }
                        }
                        Ast::FieldAccess { value, field: name } => {
                            let value_span = value.span;
                            let obj = self.eval(*value)?;
                            let value = self.eval(rhs)?;

                            let field = self.engine.get_field(&obj, value_span, &name)?;
                            let setter =
                                field
                                    .setter
                                    .as_ref()
                                    .ok_or_else(|| EvalError::ReadonlyField {
                                        ty: value.type_name_of().into(),
                                        field: name.clone(),
                                    })?;

                            setter.set(
                                value,
                                CallContext {
                                    span: name.span,
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
                            let indexer = self.engine.get_index(&value, &index, span)?;
                            let setter = indexer.setter.as_ref().ok_or_else(|| {
                                EvalError::ReadonlyIndex {
                                    ty: value.type_name_of().into(),
                                    index: index.type_name_of().into(),
                                    span,
                                }
                            })?;
                            setter.set(
                                index,
                                rhs,
                                CallContext {
                                    span,
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
                    InfixOp::Cmp(cmp) => {
                        let lhs_span = lhs.span;
                        let rhs_span = rhs.span;
                        let lhs = self.eval(lhs)?;
                        let rhs = self.eval(rhs)?;

                        let ord = if let registry = self.engine.types().get(&lhs, lhs_span)?
                            && let Some(cmp) = registry.get_cmp(Some(rhs.type_id()))
                            && let Some(ord) = cmp.apply(lhs.clone(), rhs.clone())
                        {
                            ord
                        } else if let registry = self.engine.types().get(&rhs, rhs_span)?
                            && let Some(cmp) = registry.get_cmp(Some(lhs.type_id()))
                            && let Some(ord) = cmp.apply(rhs.clone(), lhs.clone())
                        {
                            ord.reverse()
                        } else {
                            return Err(EvalError::InvalidCmp {
                                lhs: lhs.type_name_of().into(),
                                rhs: rhs.type_name_of().into(),
                                span: op_span,
                            });
                        };

                        let result = match cmp {
                            CmpOp::Lt => matches!(ord, Ordering::Less),
                            CmpOp::Lte => matches!(ord, Ordering::Less | Ordering::Equal),
                            CmpOp::Gt => matches!(ord, Ordering::Greater),
                            CmpOp::Gte => matches!(ord, Ordering::Greater | Ordering::Equal),
                            CmpOp::Eq => matches!(ord, Ordering::Equal),
                            CmpOp::NotEq => !matches!(ord, Ordering::Equal),
                        };

                        return Ok(result.into());
                    }
                    InfixOp::Add => BinOp::Add,
                    InfixOp::Sub => BinOp::Sub,
                    InfixOp::Mul => BinOp::Mul,
                    InfixOp::Div => BinOp::Div,
                };

                let lhs_span = lhs.span;
                let lhs = self.eval(lhs)?;
                let rhs = self.eval(rhs)?;

                let registry = self.engine.types().get(&lhs, lhs_span)?;

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
                if let Some(value) = value {
                    self.set_variable_by_ref(
                        var.inner.clone(),
                        LazyValueRef::lazy(self, *value),
                        true,
                    );
                } else {
                    self.set_variable_by_ref(var.inner.clone(), ValueRef::null(), true);
                }
                Ok(ValueRef::null())
            }
            Ast::FieldAccess { value, field } => {
                let value_span = value.span;
                let value = self.eval(*value)?;
                let span = field.span;
                let getter = &self.engine.get_field(&value, value_span, &field)?.getter;
                getter.get(CallContext {
                    span,
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
                let func = &self.engine.get_method(&value, value_span, &method)?;
                func.call(
                    VarArgs::new(eval_args),
                    CallContext {
                        span,
                        self_ref: value,
                    },
                )
            }
            Ast::Index { value, index } => {
                let span = index.span;
                let value = self.eval(*value)?;
                let index = self.child().eval(*index)?;
                let idx = self.engine.get_index(&value, &index, span)?;
                idx.getter.get(
                    index,
                    CallContext {
                        span,
                        self_ref: value.clone(),
                    },
                )
            }
            Ast::FunctionCall { func, args, span } => {
                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    eval_args.push(self.child().eval(a)?);
                }
                let func = self.eval(*func)?;
                let reg = self.engine.get_type(&func, span)?;
                if let Some(call) = &reg.call {
                    call.call(
                        VarArgs::new(eval_args),
                        CallContext {
                            span,
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
                let condition = self.child().eval(*condition)?;
                if condition.borrow().truthy() {
                    self.child().eval(*then)
                } else {
                    if let Some(elze) = elze {
                        self.child().eval(*elze)
                    } else {
                        Ok(ValueRef::null())
                    }
                }
            }
            Ast::ArrayLiteral { items } => Ok(items
                .into_iter()
                .map(|i| self.child().eval(i))
                .collect::<Result<Vec<_>, _>>()?
                .into()),
            Ast::ObjectLiteral { fields } => {
                let mut inner = BTreeMap::<IStr, ValueRef>::new();
                for f in fields {
                    let (k, v) = match f {
                        ObjectField::Ident(ident) => {
                            let s = ident.inner.clone();
                            let val = self.expect_variable(&ident)?;
                            (s, val)
                        }
                        ObjectField::IdentWithValue(ident, expr) => {
                            (ident.inner, self.child().eval(expr)?)
                        }
                        ObjectField::String(string_expr, expr) => {
                            (self.eval_string(string_expr)?, self.child().eval(expr)?)
                        }
                    };
                    inner.insert(k, v);
                }
                Ok(Object(inner).into())
            }
        }
    }

    fn eval_string(self: &Rc<Self>, string: StringExpr) -> EvalResult<IStr> {
        let mut out = String::new();
        let mut last = 0;
        for e in string.interpolations {
            out.push_str(&string.value[last..e.index]);
            self.eval(e.expr)?.borrow().to_string(&mut out);
            last = e.index;
        }
        out.push_str(&string.value[last..]);
        Ok(out.into())
    }
}
