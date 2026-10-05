use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, hash_map::Entry},
    ops::Not,
    rc::Rc,
};

use indexmap::IndexMap;

use crate::{
    IStr, Ident, Span, StringExt, VariableItem,
    eval::{
        Engine, EvalError, EvalResult, TypeRegistry,
        call_stack::CallStack,
        lazy::{LazyValueRef, VariableMapper, identity_mapper},
        registry::{self, BinOp, FnCtx, Function, UnaryOp, VarArgs},
        value::{
            CallContext, ValueRef,
            function::{FunctionValue, UserFunction},
            native::{Array, Null, Object},
            ty::TypeValue,
        },
    },
    expr::{self, Ast, CmpOp, Expr, InfixOp, Lit, ObjectField, ObjectFieldKV},
    parse::{Block, FunctionItem, StringExpr},
    util::OptionNonExhaustive,
};

/// Special items that are used for configuring/confirming a request
#[derive(Debug, Clone, Default, PartialEq)]
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

    /// Create a variable with a value.
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

#[derive(Clone)]
pub struct Scope {
    parent: Option<Rc<Scope>>,
    variables: RefCell<HashMap<IStr, Variable>>,
    pub(crate) engine: Rc<Engine>,
    special: RefCell<Special>,
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.variables.borrow().is_empty() && *self.special.borrow() == Special::None {
            f.write_str("Scope(<empty>)")
        } else {
            f.debug_struct("Scope")
                .field("parent", &OptionNonExhaustive(&self.parent))
                .field("variables", &self.variables.borrow())
                .field("engine", &std::fmt::from_fn(|f| f.write_str("..")))
                .field("special", &self.special.borrow())
                .finish()
        }
    }
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

    pub fn declare_function_item(self: &Rc<Self>, func: FunctionItem) {
        self.declare_variable(Variable {
            name: func.name.as_istr(),
            def: Some(func.name.clone()),
            value: LazyValueRef::from_value(
                func.name.span,
                ValueRef::new(UserFunction::new(func, self.clone())),
            ),
            readonly: true,
            on_resolve: |_, _| unreachable!(),
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
            readonly: false,
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

#[derive(Debug, Clone)]
pub enum ControlFlow {
    /// Resolve to value
    Value(ValueRef),
    /// Return from function
    Return(ValueRef),
}

/// `?` for `ControlFlow`
macro_rules! try_cf {
    ($e:expr) => {
        match $e {
            ControlFlow::Value(v) => v,
            r @ ControlFlow::Return(_) => return Ok(r),
        }
    };
}

/// Evaluation
impl Scope {
    // NOTE: This is for public API only
    pub fn eval(self: &Rc<Self>, expr: &Expr) -> EvalResult<ValueRef> {
        match self.eval_with_stack(&mut CallStack::new(), expr)? {
            ControlFlow::Value(v) => Ok(v),
            ControlFlow::Return(_) => unreachable!(),
        }
    }

    pub(crate) fn eval_with_stack(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        expr: &Expr,
    ) -> EvalResult<ControlFlow> {
        if call_stack.depth() > CallStack::MAX_DEPTH {
            return Err(EvalError::RecursionDepth { span: expr.span });
        }
        match &expr.ast {
            Ast::String(string_expr) => {
                call_stack.without_change(|cs| self.eval_string(cs, string_expr))
            }
            Ast::Lit(lit) => {
                let v = match *lit {
                    Lit::Int(n) => ValueRef::new(n as i64),
                    Lit::Float(f) => ValueRef::new(f),
                    Lit::Bool(b) => ValueRef::new(b),
                    Lit::Null => ValueRef::null(),
                };
                Ok(ControlFlow::Value(v))
            }
            Ast::Block(block) => self.eval_block(call_stack, block),
            Ast::Variable(ident) => self.expect_variable(ident).map(ControlFlow::Value),
            Ast::Request => self
                .request()
                .ok_or(EvalError::RequestInBadPosition { span: expr.span })
                .map(ControlFlow::Value),
            Ast::Response => self
                .response()
                .ok_or(EvalError::ResponseInBadPosition { span: expr.span })
                .map(ControlFlow::Value),
            Ast::PrefixOp {
                op,
                op_span,
                operand,
            } => self.eval_prefix_op(call_stack, *op, *op_span, operand),
            Ast::InfixOp {
                op,
                op_span,
                operands,
            } => call_stack.without_change(|cs| self.eval_infix(cs, *op, *op_span, operands)),
            Ast::PostfixOp { op, operand, .. } => self.eval_postfix_op(call_stack, *op, operand),
            Ast::Declare { var, value } => {
                self.declare_variable(Variable::definition(
                    self,
                    var.clone(),
                    value.as_deref().cloned(),
                ));
                Ok(ControlFlow::Value(ValueRef::null()))
            }
            Ast::FieldAccess { value, field } => self.eval_field_access(call_stack, value, field),
            Ast::MethodCall {
                value,
                method,
                args,
            } => self.eval_method_call(call_stack, value, method, args),
            Ast::Index {
                value,
                index,
                question,
            } => self.eval_index(call_stack, value, index, question.as_ref()),
            Ast::FunctionCall { func, args, span } => {
                self.eval_function_call(call_stack, func, args, *span)
            }
            Ast::Return { span, value } => {
                if call_stack.depth() == 0 {
                    Err(EvalError::InvalidReturn { span: *span })
                } else {
                    Ok(ControlFlow::Return(try_cf!(call_stack.without_change(
                        |cs| self.make_child().eval_with_stack(cs, value)
                    )?)))
                }
            }
            Ast::If {
                condition,
                then,
                elze,
            } => {
                let condition = try_cf!(
                    call_stack
                        .without_change(|cs| self.make_child().eval_with_stack(cs, condition))?
                );
                if condition.value().truthy() {
                    call_stack.without_change(|cs| self.make_child().eval_with_stack(cs, then))
                } else {
                    if let Some(elze) = elze {
                        call_stack.without_change(|cs| self.make_child().eval_with_stack(cs, elze))
                    } else {
                        Ok(ControlFlow::Value(ValueRef::null()))
                    }
                }
            }
            Ast::FunctionDef(func) => {
                self.declare_function_item(func.clone());
                Ok(ControlFlow::Value(ValueRef::null()))
            }
            Ast::ArrayLiteral { items } => {
                let mut out = Vec::new();
                for i in items {
                    out.push(try_cf!(
                        call_stack.without_change(|cs| self.make_child().eval_with_stack(cs, i))?
                    ));
                }
                Ok(ControlFlow::Value(Array::from(out).into()))
            }
            Ast::ObjectLiteral { fields } => self.eval_object_literal(call_stack, fields),
        }
    }

    fn eval_block(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        block: &Block,
    ) -> EvalResult<ControlFlow> {
        let mut last = ValueRef::null();
        let scope = self.make_child();
        for e in &block.exprs {
            last = try_cf!(call_stack.without_change(|cs| scope.eval_with_stack(cs, e))?);
        }

        let v = if block.ret { last } else { ValueRef::null() };
        Ok(ControlFlow::Value(v))
    }

    fn eval_function_call(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        func: &Expr,
        args: &[Expr],
        span: Span,
    ) -> EvalResult<ControlFlow> {
        let mut arg_spans = Vec::with_capacity(args.len());
        let mut eval_args = Vec::with_capacity(args.len());
        for a in args {
            arg_spans.push(a.span);
            eval_args.push(try_cf!(
                call_stack.without_change(|cs| self.make_child().eval_with_stack(cs, a))?
            ));
        }
        let func = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, func))?);
        let reg = self.engine.get_type(&func, span)?;
        if let Some(call) = &reg.call {
            call.call(
                VarArgs::new(eval_args),
                CallContext::new_ext(
                    span,
                    func.clone(),
                    self.clone(),
                    registry::FnCtx {
                        arg_spans,
                        call_stack,
                    },
                ),
            )
            .map(ControlFlow::Value)
        } else {
            Err(EvalError::NotCallable {
                ty: func.type_name_of().into(),
                span,
            })
        }
    }

    fn eval_index(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        value: &Expr,
        index: &Expr,
        question: Option<&Span>,
    ) -> EvalResult<ControlFlow> {
        let span = index.span;
        let ctx_ext = registry::GetIndexCtx {
            index_span: index.span,
        };
        let value = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);
        let index =
            try_cf!(call_stack.without_change(|cs| self.make_child().eval_with_stack(cs, index))?);
        let idx = self.engine.get_index(&value, &index, span)?;
        let v = idx.getter.get(
            index.clone(),
            CallContext::new_ext(span, value.clone(), self.clone(), ctx_ext),
        )?;

        if let Some(v) = v {
            Ok(ControlFlow::Value(v))
        } else if question.is_some() {
            Ok(ControlFlow::Value(ValueRef::null()))
        } else {
            Err(EvalError::MissingIndex {
                value: value.type_name_of().into(),
                index: index.display().to_string(),
                span,
            })
        }
    }

    fn eval_field_access(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        value: &Expr,
        field: &Ident,
    ) -> EvalResult<ControlFlow> {
        let value_span = value.span;
        let obj = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);
        let span = field.span;
        let reg = self.engine.get_type(&obj, value_span)?;
        if let Some(field) = reg.get_field(&field.inner) {
            field.getter.get(CallContext::new(span, obj, self.clone()))
        } else {
            reg.field_get_fallback.get(
                CallContext::new(field.span, obj, self.clone()),
                field.clone(),
            )
        }
        .map(ControlFlow::Value)
    }

    fn eval_method_call(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        value: &Expr,
        method: &Ident,
        args: &[Expr],
    ) -> EvalResult<ControlFlow> {
        let value = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);

        let mut arg_spans = Vec::with_capacity(args.len());
        let mut eval_args = Vec::with_capacity(args.len());
        for a in args {
            arg_spans.push(a.span);
            let arg =
                try_cf!(call_stack.without_change(|cs| self.make_child().eval_with_stack(cs, a))?);
            eval_args.push(arg);
        }

        let span = method.span;
        let reg = self.engine.types().get(&value, span)?;
        let ctx = CallContext::new_ext(
            span,
            value,
            self.clone(),
            registry::FnCtx {
                arg_spans,
                call_stack,
            },
        );
        reg.call_method(ctx, method.clone(), VarArgs::new(eval_args))
            .map(ControlFlow::Value)
    }

    fn eval_object_literal(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        fields: &[ObjectField],
    ) -> EvalResult<ControlFlow> {
        let mut inner = IndexMap::<IStr, ValueRef>::new();
        for f in fields {
            if let Some(cond) = &f.condition
                && !try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, cond))?)
                    .value()
                    .truthy()
            {
                continue; // skip this field
            }
            let (k, v) = match &f.kv {
                ObjectFieldKV::Ident(ident) => {
                    let s = ident.inner.clone();
                    let val = self.expect_variable(ident)?;
                    (s, val)
                }
                ObjectFieldKV::IdentWithValue(ident, expr) => (
                    ident.inner.clone(),
                    try_cf!(
                        call_stack
                            .without_change(|cs| self.make_child().eval_with_stack(cs, expr))?
                    ),
                ),
                ObjectFieldKV::String(string_expr, expr) => (
                    try_cf!(call_stack.without_change(|cs| self.eval_string(cs, string_expr))?)
                        .unwrap::<IStr>(),
                    try_cf!(
                        call_stack
                            .without_change(|cs| self.make_child().eval_with_stack(cs, expr))?
                    ),
                ),
                ObjectFieldKV::StringValue(key, value) => (key.clone(), value.clone()),
            };
            inner.insert(k, v);
        }
        Ok(ControlFlow::Value(Object(inner.into()).into()))
    }

    fn eval_prefix_op(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        op: expr::PrefixOp,
        op_span: Span,
        operand: &Expr,
    ) -> EvalResult<ControlFlow> {
        let operand = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, operand))?);
        let op = match op {
            expr::PrefixOp::Neg => UnaryOp::Prefix(registry::PrefixOp::Neg),
            expr::PrefixOp::Not => {
                return Ok(ControlFlow::Value(operand.value().truthy().not().into()));
            }
        };
        let Some(unary_op) = self.engine.types().get(&operand, op_span)?.get_unary_op(op) else {
            return Err(EvalError::InvalidUnaryOp {
                op,
                span: op_span,
                operand: operand.type_name_of().into(),
            });
        };
        unary_op
            .apply(CallContext::new(op_span, operand, self.clone()))
            .map(ControlFlow::Value)
    }

    fn eval_assign(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        op_span: Span,
        lhs: &Expr,
        rhs: &Expr,
    ) -> EvalResult<ControlFlow> {
        let rhs_span = rhs.span;
        let rhs = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, rhs))?);
        match &lhs.ast {
            Ast::Variable(var) => {
                self.set_variable(var.clone(), rhs_span, rhs)?;
            }
            Ast::FieldAccess { value, field: name } => {
                let value_span = value.span;
                let obj = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);

                let reg = self.engine.get_type(&obj, value_span)?;
                if let Some(field) = reg.get_field(&name.inner) {
                    if let Some(ref setter) = field.setter {
                        setter.set(rhs, CallContext::new(name.span, obj, self.clone()))?;
                    } else {
                        return Err(EvalError::ReadonlyField {
                            field: name.clone(),
                        });
                    }
                } else {
                    reg.field_set_fallback.set(
                        CallContext::new(name.span, obj, self.clone()),
                        name.clone(),
                        rhs,
                    )?;
                }
            }
            Ast::Index {
                value,
                index,
                question,
            } => {
                if let Some(question) = question {
                    return Err(EvalError::InvalidQuestion { span: *question });
                }
                let span = index.span;
                let ctx = registry::SetIndexCtx {
                    rhs_span,
                    index_span: index.span,
                };
                let value =
                    try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);
                let index =
                    try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, index))?);
                let indexer = self.engine.get_index(&value, &index, span)?;
                let setter = indexer
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
                    CallContext::new_ext(span, value.clone(), self.clone(), ctx),
                )?;
            }
            _ => {
                return Err(EvalError::InvalidAssignment {
                    span: op_span,
                    lhs_span: lhs.span,
                });
            }
        }
        Ok(ControlFlow::Value(ValueRef::null()))
    }

    fn eval_compound_assign(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        inner: InfixOp,
        op_span: Span,
        lhs: &Expr,
        rhs: &Expr,
    ) -> EvalResult<ControlFlow> {
        let lhs_span = lhs.span;
        let rhs_span = rhs.span;
        match &lhs.ast {
            Ast::Variable(var) => {
                let lhs = self.expect_variable(var)?;
                let rhs =
                    try_cf!(self.eval_infix_op(call_stack, inner, op_span, lhs, lhs_span, rhs)?);
                self.set_variable(var.clone(), rhs_span, rhs)?;
            }
            Ast::FieldAccess { value, field: name } => {
                let value_span = value.span;
                let obj = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);

                let reg = self.engine.get_type(&obj, value_span)?;
                if let Some(field) = reg.get_field(&name.inner) {
                    if let Some(ref setter) = field.setter {
                        let lhs = field.getter.get(CallContext::new(
                            name.span,
                            obj.clone(),
                            self.clone(),
                        ))?;
                        let rhs = try_cf!(
                            self.eval_infix_op(call_stack, inner, op_span, lhs, lhs_span, rhs)?
                        );
                        setter.set(rhs, CallContext::new(name.span, obj, self.clone()))?;
                    } else {
                        return Err(EvalError::ReadonlyField {
                            field: name.clone(),
                        });
                    }
                } else {
                    let lhs = reg.field_get_fallback.get(
                        CallContext::new(name.span, obj.clone(), self.clone()),
                        name.clone(),
                    )?;
                    let rhs = try_cf!(
                        self.eval_infix_op(call_stack, inner, op_span, lhs, lhs_span, rhs)?
                    );
                    reg.field_set_fallback.set(
                        CallContext::new(name.span, obj, self.clone()),
                        name.clone(),
                        rhs,
                    )?;
                }
            }
            Ast::Index {
                value,
                index,
                question,
            } => {
                if let Some(question) = question {
                    return Err(EvalError::InvalidQuestion { span: *question });
                }
                let index_span = index.span;
                let ctx = registry::SetIndexCtx {
                    rhs_span,
                    index_span,
                };
                let value =
                    try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, value))?);
                let index =
                    try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, index))?);
                let indexer = self.engine.get_index(&value, &index, index_span)?;

                let Some(lhs) = indexer.getter.get(
                    index.clone(),
                    CallContext::new_ext(
                        index_span,
                        value.clone(),
                        self.clone(),
                        registry::GetIndexCtx { index_span },
                    ),
                )?
                else {
                    return Err(EvalError::MissingIndex {
                        value: value.type_name_of().into(),
                        index: index.display().to_string(),
                        span: index_span,
                    });
                };
                let rhs =
                    try_cf!(self.eval_infix_op(call_stack, inner, op_span, lhs, lhs_span, rhs)?);

                let setter = indexer
                    .setter
                    .as_ref()
                    .ok_or_else(|| EvalError::ReadonlyIndex {
                        ty: value.type_name_of().into(),
                        index: index.type_name_of().into(),
                        span: index_span,
                    })?;
                setter.set(
                    index,
                    rhs,
                    CallContext::new_ext(index_span, value.clone(), self.clone(), ctx),
                )?;
            }
            _ => {
                return Err(EvalError::InvalidAssignment {
                    span: op_span,
                    lhs_span: lhs.span,
                });
            }
        };
        Ok(ControlFlow::Value(ValueRef::null()))
    }

    fn eval_infix(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        op: InfixOp,
        op_span: Span,
        (lhs, rhs): &(Expr, Expr),
    ) -> EvalResult<ControlFlow> {
        if op == InfixOp::Assign {
            return self.eval_assign(call_stack, op_span, lhs, rhs);
        } else if let Some(inner) = op.strip_assign() {
            return self.eval_compound_assign(call_stack, inner, op_span, lhs, rhs);
        }

        let lhs_span = lhs.span;
        let lhs = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, lhs))?);

        self.eval_infix_op(call_stack, op, op_span, lhs, lhs_span, rhs)
    }

    fn eval_infix_op(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        op: InfixOp,
        op_span: Span,
        lhs: ValueRef,
        lhs_span: Span,
        rhs: &Expr,
    ) -> EvalResult<ControlFlow> {
        let op = match op {
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
            | InfixOp::Assign => unreachable!("handled above"),
            InfixOp::Or => {
                if lhs.value().truthy() {
                    return Ok(ControlFlow::Value(lhs));
                } else {
                    return call_stack.without_change(|cs| self.eval_with_stack(cs, rhs));
                }
            }
            InfixOp::And => {
                if lhs.value().truthy() {
                    return call_stack.without_change(|cs| self.eval_with_stack(cs, rhs));
                } else {
                    return Ok(ControlFlow::Value(lhs));
                }
            }
            InfixOp::Cmp(cmp) => {
                let rhs_span = rhs.span;
                let rhs = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, rhs))?);

                let ord = Self::cmp(
                    &self.engine.types(),
                    op_span,
                    (&lhs, lhs_span),
                    (&rhs, rhs_span),
                )?;

                let result = match cmp {
                    CmpOp::Lt => matches!(ord, Ordering::Less),
                    CmpOp::Lte => matches!(ord, Ordering::Less | Ordering::Equal),
                    CmpOp::Gt => matches!(ord, Ordering::Greater),
                    CmpOp::Gte => matches!(ord, Ordering::Greater | Ordering::Equal),
                    CmpOp::Eq => matches!(ord, Ordering::Equal),
                    CmpOp::NotEq => !matches!(ord, Ordering::Equal),
                };

                return Ok(ControlFlow::Value(result.into()));
            }
            InfixOp::Add => BinOp::Add,
            InfixOp::Sub => BinOp::Sub,
            InfixOp::Mul => BinOp::Mul,
            InfixOp::Div => BinOp::Div,
            InfixOp::Mod => BinOp::Mod,
            InfixOp::BitAnd => BinOp::BitAnd,
            InfixOp::BitOr => BinOp::BitOr,
            InfixOp::Xor => BinOp::Xor,
            InfixOp::Shr => BinOp::Shr,
            InfixOp::Shl => BinOp::Shl,
        };

        let rhs = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, rhs))?);

        let registry = self.engine.types().get(&lhs, lhs_span)?;

        let Some(binop) = registry.get_bin_op(op, rhs.type_id()) else {
            return Err(EvalError::InvalidBinOp {
                op,
                span: op_span,
                lhs: lhs.type_name_of().into(),
                rhs: rhs.type_name_of().into(),
            });
        };

        binop
            .apply(
                lhs.clone(),
                rhs,
                CallContext::new(op_span, lhs, self.clone()),
            )
            .map(ControlFlow::Value)
    }

    fn cmp(
        types: &TypeRegistry,
        op_span: Span,
        (lhs, lhs_span): (&ValueRef, Span),
        (rhs, rhs_span): (&ValueRef, Span),
    ) -> Result<Ordering, EvalError> {
        let ord = if let registry = types.get(lhs, lhs_span)?
            && let Some(cmp) = registry.get_cmp(Some(rhs.type_id()))
            && let Some(ord) = cmp.apply(lhs.clone(), rhs.clone())
        {
            ord
        } else if let registry = types.get(rhs, rhs_span)?
            && let Some(cmp) = registry.get_cmp(Some(lhs.type_id()))
            && let Some(ord) = cmp.apply(rhs.clone(), lhs.clone())
        {
            ord.reverse()
        } else if let registry = types.get(lhs, lhs_span)?
            && let Some(cmp) = registry.get_cmp(None)
            && let Some(ord) = cmp.apply(lhs.clone(), rhs.clone())
        {
            ord
        } else if let registry = types.get(rhs, rhs_span)?
            && let Some(cmp) = registry.get_cmp(None)
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
        Ok(ord)
    }

    fn eval_postfix_op(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        op: expr::PostfixOp,
        operand: &Expr,
    ) -> EvalResult<ControlFlow> {
        let span = operand.span;
        let operand = try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, operand))?);
        match op {
            expr::PostfixOp::AssertNotNull => {
                if operand.value().is::<Null>() {
                    Err(EvalError::NotNullAssertion { span })
                } else {
                    Ok(ControlFlow::Value(operand))
                }
            }
        }
    }

    fn eval_string(
        self: &Rc<Self>,
        call_stack: &mut CallStack,
        string: &StringExpr,
    ) -> EvalResult<ControlFlow> {
        let mut out = String::new();
        let mut last = 0;
        for e in &string.interpolations {
            out.push_str(&string.value[last..e.index]);
            try_cf!(call_stack.without_change(|cs| self.eval_with_stack(cs, &e.expr))?)
                .value()
                .to_string(&mut out);
            last = e.index;
        }
        out.push_str(&string.value[last..]);
        Ok(ControlFlow::Value(out.intern().into()))
    }
}
