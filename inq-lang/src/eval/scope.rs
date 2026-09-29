use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, hash_map::Entry},
    ops::Not,
    rc::Rc,
};

use indexmap::IndexMap;

use crate::{
    IStr, Ident, Span, VariableItem,
    eval::{
        Engine, EvalError, EvalResult,
        lazy::{LazyValueRef, VariableMapper, identity_mapper},
        registry::{self, BinOp, FnCtx, Function, FunctionValue, UnaryOp, VarArgs},
        value::{
            CallContext, ValueRef,
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

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: IStr,
    /// Identifier where the definition was created
    pub def: Option<Ident>,
    pub value: LazyValueRef,
    pub readonly: bool,
    pub on_resolve: VariableMapper,
}

impl Variable {
    fn snapshot(&self) -> Self {
        Self {
            name: self.name.clone(),
            def: self.def.clone(),
            value: self.value.snapshot(),
            readonly: self.readonly,
            on_resolve: self.on_resolve,
        }
    }

    /// Create a variable with a value.  Defaults to `null` if the value is None.
    ///
    /// This variable is _not_ readonly
    pub fn definition(scope: &Scope, ident: Ident, value: Option<Expr>) -> Self {
        if let Some(value) = value {
            Self {
                name: ident.inner.clone(),
                def: Some(ident),
                value: LazyValueRef::lazy(scope, value),
                readonly: false,
                on_resolve: identity_mapper,
            }
        } else {
            let span = ident.span;
            Self {
                name: ident.inner.clone(),
                def: Some(ident),
                value: LazyValueRef::from_value(span, ValueRef::null()),
                readonly: false,
                on_resolve: identity_mapper,
            }
        }
    }

    /// Create a variable with a value.  Defaults to `null` if the value is None.
    ///
    /// This variable is _not_ readonly
    pub fn definition_resolved(ident: Ident, span: Span, value: ValueRef) -> Self {
        Self {
            name: ident.inner.clone(),
            def: Some(ident),
            value: LazyValueRef::from_value(span, value),
            readonly: false,
            on_resolve: identity_mapper,
        }
    }
}

#[derive(Default, derive_more::Debug, Clone)]
pub struct Scope {
    parent: Option<Rc<Scope>>,
    variables: RefCell<HashMap<IStr, Variable>>,
    #[debug("..")]
    pub(crate) engine: Rc<Engine>,
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

    /// Get a variable in the local scope, without going up at all
    pub(crate) fn get_local_variable(&self, name: &str) -> Option<Variable> {
        self.variables.borrow().get(name).cloned()
    }

    pub fn get_variable(&self, name: &str) -> Option<Variable> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Some(var.clone())
        } else if let Some(parent) = &self.parent {
            parent.get_variable(name)
        } else {
            None
        }
    }

    pub fn resolve_var(&self, name: &str) -> EvalResult<Option<ValueRef>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Ok(Some(var.value.get()?))
        } else if let Some(parent) = &self.parent {
            parent.resolve_var(name)
        } else {
            Ok(None)
        }
    }

    /// Get a variable _only_ if it has been evaluated already
    pub fn get_evaluated_variable(&self, name: &str) -> EvalResult<Option<ValueRef>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            Ok(var.value.resolved())
        } else if let Some(parent) = &self.parent {
            parent.get_evaluated_variable(name)
        } else {
            Ok(None)
        }
    }

    /// Get a variable _only_ if it has been evaluated already
    // this name is sad
    pub fn get_evaluated_variable_with_span(
        &self,
        name: &str,
    ) -> EvalResult<Option<(ValueRef, Span)>> {
        let vars = self.variables.borrow();
        if let Some(var) = vars.get(name) {
            let Some(resolved) = var.value.resolved() else {
                return Ok(None);
            };
            Ok(Some((resolved, var.value.value_span())))
        } else if let Some(parent) = &self.parent {
            parent.get_evaluated_variable_with_span(name)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn expect_variable(&self, ident: &Ident) -> EvalResult<ValueRef> {
        if let Some(var) = self.resolve_var(ident.as_ref())? {
            return Ok(var);
        }

        if let Some(ty) = self.engine.types.borrow().get_by_name(&ident.inner) {
            return Ok(TypeValue::from(ty).into());
        }

        Err(EvalError::UndefinedVariable {
            ident: ident.clone(),
        })
    }

    pub fn declare_function<V, R>(
        &self,
        name: impl Into<IStr>,
        func: fn(CallContext<FnCtx>, V) -> R,
    ) where
        fn(CallContext<FnCtx>, V) -> R: Function + 'static,
    {
        self.declare_variable(Variable {
            name: name.into(),
            def: None,
            value: LazyValueRef::from_value(Span::empty(), FunctionValue::new(func).into()),
            readonly: true,
            on_resolve: identity_mapper,
        });
    }

    /// Declare a new variable, returning an error if it already exists in the current scope
    pub fn declare_variable(&self, var: Variable) {
        self.variables.borrow_mut().insert(var.name.clone(), var);
    }

    /// Set the value of a variable, erroring if the variable does not exist
    pub fn set_variable(&self, name: Ident, span: Span, value: ValueRef) -> EvalResult<()> {
        let mut vars = self.variables.borrow_mut();
        match vars.entry(name.inner.clone()) {
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                if v.readonly {
                    return Err(EvalError::ReadonlyVariable {
                        call: name,
                        definition: v.def.as_ref().map(|i| i.span),
                    });
                } else {
                    v.value = LazyValueRef::from_value(span, (v.on_resolve)(span, value)?);
                }
            }
            Entry::Vacant(_) if let Some(parent) = &self.parent => {
                return parent.set_variable(name, span, value);
            }
            Entry::Vacant(_) => return Err(EvalError::UndefinedVariable { ident: name }),
        }
        Ok(())
    }

    /// Set the value of a variable, erroring if the variable does not exist
    pub fn set_variable_lazy(&self, name: Ident, expr: Expr) -> EvalResult<()> {
        let mut vars = self.variables.borrow_mut();
        match vars.entry(name.inner.clone()) {
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.value = LazyValueRef::lazy_mapped(self, expr, v.on_resolve);
            }
            Entry::Vacant(_) if let Some(parent) = &self.parent => {
                return parent.set_variable_lazy(name, expr);
            }
            Entry::Vacant(_) => return Err(EvalError::UndefinedVariable { ident: name }),
        }
        Ok(())
    }

    pub fn add_variable(&self, var: VariableItem) {
        self.add_mapped_variable(var, identity_mapper);
    }

    /// Add a readonly variable from an item with an optional mapper
    pub fn add_mapped_variable(&self, var: VariableItem, mapper: VariableMapper) {
        self.declare_variable(Variable {
            name: var.name.inner.clone(),
            def: Some(var.name),
            value: LazyValueRef::lazy_mapped(self, var.value, mapper),
            readonly: true,
            on_resolve: mapper,
        });
    }

    pub fn set_special(&self, special: Special) {
        self.special.replace(special);
    }

    pub fn special(&self) -> Option<Special> {
        match &*self.special.borrow() {
            Special::None => self.parent.as_deref().and_then(Scope::special),
            spec => Some(spec.clone()),
        }
    }

    pub(crate) fn request(&self) -> Option<ValueRef> {
        match self.special()? {
            Special::None => unreachable!(),
            Special::Request(r) => Some(r),
            Special::Response(_) => None,
        }
    }

    pub(crate) fn response(&self) -> Option<ValueRef> {
        match self.special()? {
            Special::None => unreachable!(),
            Special::Request(_) => None,
            Special::Response(r) => Some(r),
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
                unary_op.apply(CallContext::new(op_span, operand, self.clone()))
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
                            let rhs_span = rhs.span;
                            let rhs = self.eval(rhs)?;
                            self.set_variable(var, rhs_span, rhs)?;
                            Ok(ValueRef::null())
                        }
                        Ast::FieldAccess { value, field: name } => {
                            let value_span = value.span;
                            let obj = self.eval(*value)?;
                            let value = self.eval(rhs)?;

                            let reg = self.engine.get_type(&obj, value_span)?;
                            if let Some(field) = reg.get_field(&name.inner) {
                                if let Some(ref setter) = field.setter {
                                    setter.set(
                                        value,
                                        CallContext::new(name.span, obj, self.clone()),
                                    )?;
                                } else {
                                    return Err(EvalError::ReadonlyField {
                                        ty: value.type_name_of().into(),
                                        field: name.clone(),
                                    });
                                }
                            } else {
                                reg.field_set_fallback.set(
                                    CallContext::new(name.span, obj, self.clone()),
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
                                CallContext::new_ext(span, value.clone(), self.clone(), ctx),
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

                binop.apply(
                    lhs.clone(),
                    rhs,
                    CallContext::new(op_span, lhs, self.clone()),
                )
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
                self.declare_variable(Variable::definition(self, var, value.map(|b| *b)));
                Ok(ValueRef::null())
            }
            Ast::FieldAccess { value, field } => {
                let value_span = value.span;
                let obj = self.eval(*value)?;
                let span = field.span;
                let reg = self.engine.get_type(&obj, value_span)?;
                if let Some(field) = reg.get_field(&field.inner) {
                    field.getter.get(CallContext::new(span, obj, self.clone()))
                } else {
                    reg.field_get_fallback
                        .get(CallContext::new(field.span, obj, self.clone()), field)
                }
            }
            Ast::MethodCall {
                value,
                method,
                args,
            } => {
                let value = self.eval(*value)?;

                let mut arg_spans = Vec::with_capacity(args.len());
                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    arg_spans.push(a.span);
                    eval_args.push(self.make_child().eval(a)?);
                }

                let span = method.span;
                let reg = self.engine.types().get(&value, span)?;
                let ctx =
                    CallContext::new_ext(span, value, self.clone(), registry::FnCtx { arg_spans });
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
                    CallContext::new_ext(span, value.clone(), self.clone(), ctx_ext),
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
                let mut arg_spans = Vec::with_capacity(args.len());
                let mut eval_args = Vec::with_capacity(args.len());
                for a in args {
                    arg_spans.push(a.span);
                    eval_args.push(self.make_child().eval(a)?);
                }
                let func = self.eval(*func)?;
                let reg = self.engine.get_type(&func, span)?;
                if let Some(call) = &reg.call {
                    call.call(
                        VarArgs::new(eval_args),
                        CallContext::new_ext(
                            span,
                            func.clone(),
                            self.clone(),
                            registry::FnCtx { arg_spans },
                        ),
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
