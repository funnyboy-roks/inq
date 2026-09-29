use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use miette::{NamedSource, Severity};

use crate::{
    IStr, Ident, Item,
    eval::{EvalError, EvalResult, Scope, Special, Variable, value::ValueRef},
    expr::{Ast, Expr, ObjectField},
    parse::Block,
};

/// Checker that walks ast and checks whether things are correct.
///
/// At the moment, this just checks for undefined and unused variables, but the hope is to expand it
/// in the future, perhaps to type-checking.
#[derive(Clone)]
pub struct Checker {
    parent: Option<Rc<Checker>>,
    scope: Rc<Scope>,
    defined_variables: Rc<RefCell<HashSet<(Ident, usize)>>>,
    used_variables: Rc<RefCell<HashSet<(Ident, usize)>>>,
    /// Currently defined variables.  When a new one in the current scope with the same name is
    /// defined, the value should be updated.
    ///
    /// ```
    /// let hello = 42; // (hello, 0)
    /// let hello = 27; // (hello, 1)
    /// ```
    current_variables: Rc<RefCell<HashMap<Ident, usize>>>,
    source: NamedSource<IStr>,
}

impl Drop for Checker {
    fn drop(&mut self) {
        let defined_variables = self.defined_variables.take();
        let used_variables = self.used_variables.take();

        // TODO: report as a single error, if possible
        for unused in defined_variables.difference(&used_variables) {
            let err = miette::miette! {
                labels = vec![unused.0.span.with_label("Defined here")],
                severity = Severity::Warning,
                "Unused Variable: '{}'", unused.0,
            };
            eprintln!("{:?}", err.with_source_code(self.source.clone()));
        }
    }
}

// eval, but without completing any work.
impl Checker {
    pub fn new(scope: Rc<Scope>, source: NamedSource<IStr>) -> Rc<Self> {
        Rc::new(Self {
            parent: None,
            scope,
            defined_variables: Default::default(),
            used_variables: Default::default(),
            current_variables: Default::default(),
            source,
        })
    }

    pub fn check_item(self: &Rc<Self>, item: &Item) -> EvalResult<()> {
        match item {
            Item::Variable(v) => {
                self.check(&v.value)?;
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

    fn make_child(self: &Rc<Self>) -> Rc<Self> {
        Rc::new(Self {
            parent: Some(self.clone()),
            scope: self.scope.make_child(),
            defined_variables: Default::default(),
            used_variables: Default::default(),
            current_variables: Default::default(),
            source: self.source.clone(),
        })
    }

    pub(crate) fn expect_variable(&self, ident: &Ident) -> EvalResult<()> {
        if self.scope.get_local_variable(&ident.inner).is_some() {
            let n = self
                .current_variables
                .borrow()
                .get(ident)
                .copied()
                .unwrap_or_default();
            self.used_variables.borrow_mut().insert((ident.clone(), n));
            Ok(())
        } else if let Some(parent) = &self.parent {
            parent.expect_variable(ident)
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
            .current_variables
            .borrow_mut()
            .entry(ident.clone())
            .and_modify(|n| *n += 1)
            .or_insert(0);
        self.defined_variables
            .borrow_mut()
            .insert((ident.clone(), n));
    }

    fn check_block(self: &Rc<Self>, block: &Block) -> EvalResult<()> {
        let child = self.make_child();
        for e in &block.exprs {
            child.check(e)?;
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
                self.expect_variable(ident)?;
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
            Ast::InfixOp { operands, .. } => {
                self.check(&operands.0)?;
                self.check(&operands.1)?;
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
            Ast::ArrayLiteral { items } => {
                for a in items {
                    self.check(a)?;
                }
            }
            Ast::ObjectLiteral { fields } => {
                for f in fields {
                    match f {
                        ObjectField::Ident(ident) => {
                            self.expect_variable(ident)?;
                        }
                        ObjectField::IdentWithValue(_, expr) => {
                            self.make_child().check(expr)?;
                        }
                        ObjectField::String(s, expr) => {
                            for i in &s.interpolations {
                                self.check(&i.expr)?;
                            }
                            self.make_child().check(expr)?;
                        }
                        ObjectField::StringValue(_, _) => {}
                    }
                }
            }
        }
        Ok(())
    }
}
