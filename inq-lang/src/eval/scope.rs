use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, hash_map::Entry},
    ops::Not,
    rc::Rc,
};

use indexmap::IndexMap;

use crate::{
    IStr, Ident, Variable,
    eval::{
        Engine, EvalError, EvalResult,
        lazy::LazyValueRef,
        registry::{self, BinOp, UnaryOp, VarArgs},
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Null, Object},
            ty::TypeValue,
        },
    },
    expr::{Ast, CmpOp, Expr, InfixOp, Lit, ObjectField},
    parse::StringExpr,
};

/// Special items that are used for configuring/confirming a request
#[derive(Debug, Clone, Default)]
pub enum Special {
    #[default]
    None,
    /// The `request` special variable
    Request(ValueRef),
    /// The `response` special variable
    Response(ValueRef),
}

impl Special {
    fn snapshot(&self) -> Self {
        match self {
            Special::None => Special::None,
            Special::Request(r) => Special::Request(r.snapshot()),
            Special::Response(r) => Special::Response(r.snapshot()),
        }
    }
}

#[derive(Default, derive_more::Debug, Clone)]
pub struct Scope {
    parent: Option<Rc<Scope>>,
    variables: RefCell<HashMap<IStr, LazyValueRef>>,
    #[debug("..")]
    engine: Rc<Engine>,
    special: RefCell<Special>,
}

impl Scope {
    pub(super) fn new(engine: Rc<Engine>) -> Self {
        Self {
            parent: None,
            variables: Default::default(),
            engine,
            special: Default::default(),
        }
    }

    /// Take a snapshot of this scope such that modifying `self` does not modify the returned
    /// snapshot
    ///
    /// The only exception is that the engine itself is the same
    pub(crate) fn snapshot(&self) -> Self {
        Self {
            parent: self.parent.as_ref().map(|s| Rc::new(s.snapshot())),
            variables: RefCell::new(
                self.variables
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.snapshot()))
                    .collect(),
            ),
            engine: self.engine.clone(),
            special: self.special.borrow().snapshot().into(),
        }
    }

    pub fn make_child(self: &Rc<Self>) -> Rc<Self> {
        let mut this = Self::new(self.engine.clone());
        this.parent = Some(self.clone());
        this.into()
    }

    pub fn get_lazy_variable(&self, name: &str) -> EvalResult<Option<LazyValueRef>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Ok(Some(var.clone()))
        } else if let Some(parent) = &self.parent {
            parent.get_lazy_variable(name)
        } else {
            Ok(None)
        }
    }

    pub fn get_variable(&self, name: &str) -> EvalResult<Option<ValueRef>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Ok(Some(var.get()?))
        } else if let Some(parent) = &self.parent {
            parent.get_variable(name)
        } else {
            Ok(None)
        }
    }

    /// Get a variable _only_ if it has been evaluated already
    pub fn get_evaluated_variable(&self, name: &str) -> EvalResult<Option<ValueRef>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Ok(var.resolved())
        } else if let Some(parent) = &self.parent {
            parent.get_evaluated_variable(name)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn expect_variable(&self, ident: &Ident) -> EvalResult<ValueRef> {
        if let Some(var) = self.get_variable(ident.as_ref())? {
            return Ok(var);
        }

        if let Some(ty) = self.engine.types.borrow().get_by_name(&ident.inner) {
            return Ok(TypeValue::from(ty).into());
        }

        Err(EvalError::UndefinedVariable {
            ident: ident.clone(),
        })
    }

    /// Returns true if the variable already existed in the current scope
    pub fn set_variable<V>(&self, name: impl Into<IStr>, value: V, declare: bool) -> bool
    where
        V: Value,
    {
        self.engine.register_type::<V>();
        self.set_variable_ref(name, value, declare)
    }

    /// Returns true if the variable already existed in the current scope
    pub fn set_variable_ref<V>(&self, name: impl Into<IStr>, value: V, declare: bool) -> bool
    where
        V: Into<ValueRef>,
    {
        self.set_variable_by_ref(name, value.into(), declare)
    }

    pub fn add_variable(&self, var: Variable) {
        self.set_variable_by_ref(var.name.inner, LazyValueRef::lazy(self, var.value), true);
    }

    pub fn set_special(&self, special: Special) {
        self.special.replace(special);
    }

    fn special(&self) -> Option<Special> {
        match &*self.special.borrow() {
            Special::None => self.parent.as_deref().and_then(Scope::special),
            spec => Some(spec.clone()),
        }
    }

    fn request(&self) -> Option<ValueRef> {
        match self.special()? {
            Special::None => unreachable!(),
            Special::Request(r) => Some(r),
            Special::Response(_) => None,
        }
    }

    fn response(&self) -> Option<ValueRef> {
        match self.special()? {
            Special::None => unreachable!(),
            Special::Request(_) => None,
            Special::Response(r) => Some(r),
        }
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
                let scope = self.make_child();
                for e in block.exprs {
                    last = scope.eval(e)?;
                }
                if block.ret {
                    Ok(last)
                } else {
                    Ok(ValueRef::null())
                }
            }
            Ast::Variable(ident) => self.expect_variable(&ident),
            Ast::Request => self
                .request()
                .ok_or(EvalError::RequestInBadPosition { span: expr.span }),
            Ast::Response => self
                .response()
                .ok_or(EvalError::ResponseInBadPosition { span: expr.span }),
            Ast::PrefixOp {
                op,
                op_span,
                operand,
            } => {
                let operand = self.eval(*operand)?;
                let op = match op {
                    crate::expr::PrefixOp::Neg => UnaryOp::Prefix(registry::PrefixOp::Neg),
                    crate::expr::PrefixOp::Not => {
                        return Ok(operand.value().truthy().not().into());
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
                unary_op.apply(CallContext::new(op_span, operand))
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

                            let reg = self.engine.get_type(&obj, value_span)?;
                            if let Some(field) = reg.get_field(&name.inner) {
                                if let Some(ref setter) = field.setter {
                                    setter.set(value, CallContext::new(name.span, obj))?;
                                } else {
                                    return Err(EvalError::ReadonlyField {
                                        ty: value.type_name_of().into(),
                                        field: name.clone(),
                                    });
                                }
                            } else {
                                reg.field_set_fallback.set(
                                    CallContext::new(name.span, obj),
                                    name,
                                    value,
                                )?;
                            }

                            Ok(ValueRef::null())
                        }
                        Ast::Index {
                            value,
                            index,
                            question,
                        } => {
                            if let Some(question) = question {
                                return Err(EvalError::InvalidQuestion { span: question });
                            }
                            let span = index.span;
                            let ctx = registry::SetIndexCtx {
                                rhs_span: rhs.span,
                                index_span: index.span,
                            };
                            let rhs = self.eval(rhs)?;
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
                                CallContext::new_ext(span, value.clone(), ctx),
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
                        if lhs.value().truthy() {
                            return Ok(lhs);
                        } else {
                            return self.eval(rhs);
                        }
                    }
                    InfixOp::And => {
                        let lhs = self.eval(lhs)?;
                        if lhs.value().truthy() {
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

                binop.apply(lhs.clone(), rhs, CallContext::new(op_span, lhs))
            }
            Ast::PostfixOp { op, operand, .. } => {
                let span = operand.span;
                let operand = self.eval(*operand)?;
                match op {
                    crate::expr::PostfixOp::AssertNotNull => {
                        if operand.value().is::<Null>() {
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
                let obj = self.eval(*value)?;
                let span = field.span;
                let reg = self.engine.get_type(&obj, value_span)?;
                if let Some(field) = reg.get_field(&field.inner) {
                    field.getter.get(CallContext::new(span, obj))
                } else {
                    reg.field_get_fallback
                        .get(CallContext::new(field.span, obj), field)
                }
            }
            Ast::MethodCall {
                value,
                method,
                args,
            } => {
                let value = self.eval(*value)?;

                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    eval_args.push(self.eval(a)?);
                }

                let span = method.span;
                let reg = self.engine.types().get(&value, span)?;
                let ctx = CallContext::new(span, value);
                reg.call_method(ctx, method, VarArgs::new(eval_args))
            }
            Ast::Index {
                value,
                index,
                question,
            } => {
                let span = index.span;
                let ctx_ext = registry::GetIndexCtx {
                    index_span: index.span,
                };
                let value = self.eval(*value)?;
                let index = self.make_child().eval(*index)?;
                let idx = self.engine.get_index(&value, &index, span)?;
                let v = idx.getter.get(
                    index.clone(),
                    CallContext::new_ext(span, value.clone(), ctx_ext),
                )?;

                if let Some(v) = v {
                    Ok(v)
                } else if question.is_some() {
                    Ok(ValueRef::null())
                } else {
                    Err(EvalError::MissingIndex {
                        value: value.type_name_of().into(),
                        index: index.display().to_string(),
                        span,
                    })
                }
            }
            Ast::FunctionCall { func, args, span } => {
                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    eval_args.push(self.make_child().eval(a)?);
                }
                let func = self.eval(*func)?;
                let reg = self.engine.get_type(&func, span)?;
                if let Some(call) = &reg.call {
                    call.call(
                        VarArgs::new(eval_args),
                        CallContext::new(span, func.clone()),
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
                let condition = self.make_child().eval(*condition)?;
                if condition.value().truthy() {
                    self.make_child().eval(*then)
                } else {
                    if let Some(elze) = elze {
                        self.make_child().eval(*elze)
                    } else {
                        Ok(ValueRef::null())
                    }
                }
            }
            Ast::ArrayLiteral { items } => items
                .into_iter()
                .map(|i| self.make_child().eval(i))
                .collect::<Result<Array, _>>()
                .map(Into::into),
            Ast::ObjectLiteral { fields } => {
                let mut inner = IndexMap::<IStr, ValueRef>::new();
                for f in fields {
                    let (k, v) = match f {
                        ObjectField::Ident(ident) => {
                            let s = ident.inner.clone();
                            let val = self.expect_variable(&ident)?;
                            (s, val)
                        }
                        ObjectField::IdentWithValue(ident, expr) => {
                            (ident.inner, self.make_child().eval(expr)?)
                        }
                        ObjectField::String(string_expr, expr) => (
                            self.eval_string(string_expr)?,
                            self.make_child().eval(expr)?,
                        ),
                        ObjectField::StringValue(key, value) => (key, value),
                    };
                    inner.insert(k, v);
                }
                Ok(Object(inner.into()).into())
            }
        }
    }

    fn eval_string(self: &Rc<Self>, string: StringExpr) -> EvalResult<IStr> {
        let mut out = String::new();
        let mut last = 0;
        for e in string.interpolations {
            out.push_str(&string.value[last..e.index]);
            self.eval(e.expr)?.value().to_string(&mut out);
            last = e.index;
        }
        out.push_str(&string.value[last..]);
        Ok(out.into())
    }
}
