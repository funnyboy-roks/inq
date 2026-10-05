use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

use indexmap::IndexSet;
use miette::{NamedSource, Severity};

use crate::{
    IStr, Ident, Item,
    eval::{EvalError, EvalResult, Scope, Special, Variable, value::ValueRef},
    expr::{Ast, Expr, InfixOp, ObjectFieldKV},
    parse::{Block, FunctionWith},
    util::OptionNonExhaustive,
};

#[derive(Debug, Clone, Copy)]
struct VarCheck {
    used: bool,
}

struct CheckScope {
    checker: Weak<Checker>,
    parent: Option<Rc<Self>>,
    child: RefCell<Vec<Rc<Self>>>,
    scope: Rc<Scope>,
    /// Variables _defined_ in this scope.  Value is effectively a key for Checker.varibles
    variables: Rc<RefCell<IndexSet<(Ident, usize)>>>,
    in_func: Cell<bool>,
}

impl std::fmt::Debug for CheckScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.variables.borrow().is_empty() && self.child.borrow().is_empty() {
            f.write_str("CheckScope(<empty>)")
        } else {
            let dd = std::fmt::from_fn(|f| f.write_str(".."));
            f.debug_struct("CheckScope")
                .field("checker", &dd)
                .field("parent", &OptionNonExhaustive(&self.parent))
                .field("child", &self.child.borrow())
                .field("scope", &self.scope)
                .field("variables", &self.variables.borrow())
                .finish()
        }
    }
}

/// Checker that walks ast and checks whether things are correct.
///
/// At the moment, this just checks for undefined and unused variables, but the hope is to expand it
/// in the future, perhaps to type-checking.
#[derive(Debug)]
pub struct Checker {
    source: NamedSource<IStr>,
    root: Rc<CheckScope>,
    variable_index: RefCell<HashMap<Ident, usize>>,
    variables: RefCell<HashMap<(Ident, usize), VarCheck>>,
}

impl Checker {
    pub fn new(scope: Rc<Scope>, source: NamedSource<IStr>) -> Rc<Self> {
        Rc::new_cyclic(|this| Self {
            source,
            root: CheckScope::new(scope, this.clone()),
            variable_index: Default::default(),
            variables: Default::default(),
        })
    }

    pub fn check_item(&self, item: &Item) -> EvalResult<()> {
        self.root.check_item(item)
    }

    pub fn print_warnings(&self) {
        self.root.print_warnings();
    }
}

/// Walk and collect the variables
impl CheckScope {
    fn new(scope: Rc<Scope>, checker: Weak<Checker>) -> Rc<Self> {
        Rc::new(Self {
            checker,
            parent: None,
            child: RefCell::new(Vec::new()),
            variables: Default::default(),
            scope,
            in_func: Cell::new(false),
        })
    }

    fn checker(&self) -> Rc<Checker> {
        self.checker.upgrade().expect("Only fails in ::new")
    }

    fn make_child(self: &Rc<Self>) -> Rc<Self> {
        let new = Rc::new(Self {
            checker: self.checker.clone(),
            parent: Some(self.clone()),
            child: RefCell::new(Vec::new()),
            scope: self.scope.make_child(),
            variables: Default::default(),
            in_func: Cell::new(false),
        });
        self.child.borrow_mut().push(new.clone());
        new
    }

    /// Expect a variable to exist.  If `usage` is true, it will be counted as a usage
    fn expect_variable(&self, ident: &Ident, usage: bool) -> EvalResult<()> {
        if self.scope.get_local_variable(&ident.inner).is_some() {
            let n = self
                .checker()
                .variable_index
                .borrow()
                .get(ident)
                .copied()
                .unwrap_or_default();
            if usage
                && let Some(var) = self
                    .checker()
                    .variables
                    .borrow_mut()
                    .get_mut(&(ident.clone(), n))
            {
                // None if defined externally
                var.used = true;
            }
            Ok(())
        } else if let Some(parent) = &self.parent {
            parent.expect_variable(ident, usage)
        } else if self.scope.engine.get_type_by_name(&ident.inner).is_some() {
            Ok(()) // type, it's fine
        } else {
            Err(EvalError::UndefinedVariable {
                ident: ident.clone(),
            })
        }
    }

    pub(crate) fn declare_variable(&self, ident: &Ident) {
        self.scope.declare_variable(Variable::definition_resolved(
            ident.clone(),
            ident.span,
            ValueRef::null(),
        ));
        let &mut n = self
            .checker()
            .variable_index
            .borrow_mut()
            .entry(ident.clone())
            .and_modify(|n| *n += 1)
            .or_insert(0);
        self.variables.borrow_mut().insert((ident.clone(), n));
        self.checker()
            .variables
            .borrow_mut()
            .insert((ident.clone(), n), VarCheck { used: false });
    }

    fn check_block(self: &Rc<Self>, block: &Block) -> EvalResult<()> {
        let child = self.make_child();
        for e in &block.exprs {
            child.check(e)?;
        }
        Ok(())
    }

    fn check_item(self: &Rc<Self>, item: &Item) -> EvalResult<()> {
        match item {
            Item::Variable(v) => {
                self.check(&v.value)?;
                self.declare_variable(&v.name);
            }
            Item::Function(func) => {
                let child = self.make_child();
                match func.with {
                    Some((FunctionWith::Request, _)) => {
                        child.scope.set_special(Special::Request(ValueRef::null()))
                    }
                    Some((FunctionWith::Response, _)) => {
                        child.scope.set_special(Special::Response(ValueRef::null()))
                    }
                    None => {}
                }
                for a in &func.args {
                    child.declare_variable(a);
                }
                child.check(&func.body)?;
                self.declare_variable(&func.name);
            }
            Item::Route(route) => {
                let child = self.make_child();
                for a in &route.args {
                    if let Some(default) = &a.default_value {
                        child.check(default)?;
                    }
                }
                for a in &route.args {
                    child.declare_variable(&a.name);
                }
                child.check(&route.endpoint)?;
                if let Some(before) = &route.before {
                    let b = child.make_child();
                    b.scope.set_special(Special::Request(ValueRef::null()));
                    b.check_block(before)?;
                }
                if let Some(after) = &route.after {
                    let b = child.make_child();
                    b.scope.set_special(Special::Response(ValueRef::null()));
                    b.check_block(after)?;
                }
            }
        }
        Ok(())
    }

    pub fn check(self: &Rc<Self>, expr: &Expr) -> EvalResult<()> {
        match &expr.ast {
            Ast::String(s) => {
                for i in &s.interpolations {
                    self.check(&i.expr)?;
                }
            }
            Ast::Lit(_) => {} // always okay
            Ast::Block(block) => self.check_block(block)?,
            Ast::Variable(ident) => {
                self.expect_variable(ident, true)?;
            }
            Ast::Request => {
                self.scope
                    .request()
                    .ok_or(EvalError::RequestInBadPosition { span: expr.span })?;
            }
            Ast::Response => {
                self.scope
                    .response()
                    .ok_or(EvalError::ResponseInBadPosition { span: expr.span })?;
            }
            Ast::PrefixOp { operand, .. } => self.check(operand)?,
            Ast::InfixOp { operands, op, .. } => {
                let (lhs, rhs) = &**operands;
                self.check(rhs)?;
                if *op == InfixOp::Assign {
                    match &lhs.ast {
                        Ast::Variable(var) => {
                            self.expect_variable(var, false)?;
                        }
                        Ast::FieldAccess { value, .. } => {
                            self.check(value)?;
                        }
                        Ast::Index { value, index, .. } => {
                            self.check(value)?;
                            self.check(index)?;
                        }
                        _ => {
                            self.check(lhs)?;
                        }
                    };
                } else {
                    self.check(lhs)?;
                }
            }
            Ast::PostfixOp { operand, .. } => self.check(operand)?,
            Ast::Declare { var, value } => {
                self.declare_variable(var);
                if let Some(value) = value {
                    self.check(value)?;
                }
            }
            Ast::FieldAccess { value, .. } => self.check(value)?,
            Ast::MethodCall { value, args, .. } => {
                self.check(value)?;
                for a in args {
                    self.check(a)?;
                }
            }
            Ast::Index { value, index, .. } => {
                self.check(value)?;
                self.check(index)?;
            }
            Ast::FunctionCall { func, args, .. } => {
                self.check(func)?;
                for a in args {
                    self.check(a)?;
                }
            }
            Ast::If {
                condition,
                then,
                elze,
            } => {
                self.check(condition)?;
                self.check(then)?;
                if let Some(elze) = elze {
                    self.check(elze)?;
                }
            }
            Ast::FunctionDef(func) => {
                let child = self.make_child();
                child.in_func.set(true);
                match func.with {
                    Some((FunctionWith::Request, _)) => {
                        child.scope.set_special(Special::Request(ValueRef::null()))
                    }
                    Some((FunctionWith::Response, _)) => {
                        child.scope.set_special(Special::Response(ValueRef::null()))
                    }
                    None => {}
                }
                for a in &func.args {
                    child.declare_variable(a);
                }
                child.check(&func.body)?;
                self.declare_variable(&func.name);
            }
            Ast::Return { span, value } => {
                if self.in_func.get() {
                    self.check(value)?;
                } else {
                    return Err(EvalError::InvalidReturn { span: *span });
                }
            }
            Ast::ArrayLiteral { items } => {
                for a in items {
                    self.check(a)?;
                }
            }
            Ast::ObjectLiteral { fields } => {
                for f in fields {
                    if let Some(cond) = &f.condition {
                        self.check(cond)?;
                    }
                    match &f.kv {
                        ObjectFieldKV::Ident(ident) => {
                            self.expect_variable(ident, true)?;
                        }
                        ObjectFieldKV::IdentWithValue(_, expr) => {
                            self.make_child().check(expr)?;
                        }
                        ObjectFieldKV::String(s, expr) => {
                            for i in &s.interpolations {
                                self.check(&i.expr)?;
                            }
                            self.make_child().check(expr)?;
                        }
                        ObjectFieldKV::StringValue(_, _) => {}
                    }
                }
            }
        }
        Ok(())
    }
}

/// Displaying errors to the user
///
/// This is intentionally poorly optimised because it's still hella fast and error should be rare
impl CheckScope {
    fn should_warn_about_name(name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        match name {
            _ if name.starts_with('_') => false,
            "BASE_URL" => false, // implied usage, so won't be caught
            _ => true,
        }
    }

    fn print_warnings(&self) {
        for id @ &(ref name, n) in &*self.variables.borrow() {
            if let Some(varchk) = self.checker().variables.borrow().get(id)
                && !varchk.used
                && Self::should_warn_about_name(&name.inner)
            {
                let shadow = self.var_shadowed_by_unused(name, n);

                if shadow.is_empty() {
                    let err = miette::miette! {
                        labels = vec![name.span.with_label("Defined here")],
                        help = format!("Remove this variable or change it to '_{}' to remove this warning", name),
                        severity = Severity::Warning,
                        "Unused Variable: '{}'", name,
                    };
                    eprintln!("{:?}", err.with_source_code(self.checker().source.clone()));
                } else {
                    let mut labels = vec![name.span.with_label_primary("Defined here")];
                    labels.extend(
                        shadow
                            .into_iter()
                            .map(|(name, _, _)| name.span.with_label("Variable shadowed here")),
                    );
                    let err = miette::miette! {
                        labels = labels,
                        help = format!("Remove this variable or change it to '_{}' to remove this warning", name),
                        severity = Severity::Warning,
                        "Unused Variable: '{}'", name,
                    };
                    eprintln!("{:?}", err.with_source_code(self.checker().source.clone()));
                }
            }
        }
        for child in &*self.child.borrow() {
            child.print_warnings();
        }
    }

    fn var_shadowed_by_unused(&self, ident: &Ident, n: usize) -> Vec<(Ident, usize, VarCheck)> {
        self.checker()
            .variables
            .borrow()
            .iter()
            .filter(|(k, v)| k.0 == *ident && k.1 > n && !v.used)
            .map(|((name, n), v)| (name.clone(), *n, *v))
            .collect()
    }
}
