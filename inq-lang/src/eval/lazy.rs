use std::{cell::RefCell, rc::Rc};

use crate::{
    Span,
    eval::{
        EvalResult, Scope,
        value::{Value, ValueRef},
    },
    expr::Expr,
};

#[derive(Clone, derive_more::Debug)]
enum LazyValueRefInner {
    Resolved {
        resolved: ValueRef,
        span: Span,
    },
    #[debug("Pending({})", expr.clone().unwrap())]
    Pending {
        /// Snapshot of the scope at the time the variable was set
        scope_snapshot: Rc<Scope>,
        /// Option just so it can be taken
        expr: Option<Expr>,
    },
}

#[derive(derive_more::Debug, Clone)]
#[debug("{:?}", inner.borrow())]
pub struct LazyValueRef {
    inner: Rc<RefCell<LazyValueRefInner>>,
}

impl LazyValueRef {
    pub fn get(&self) -> EvalResult<ValueRef> {
        let mut guard = self.inner.borrow_mut();
        match &mut *guard {
            LazyValueRefInner::Resolved { resolved, .. } => Ok(resolved.clone()),
            LazyValueRefInner::Pending {
                scope_snapshot,
                expr,
            } => {
                let expr = expr.take().expect(
                    "This take sets inner to resolved and this option is not taken anywhere else",
                );

                let span = expr.span;
                let val = scope_snapshot.eval(expr)?;
                drop(guard);
                let mut inner = self.inner.borrow_mut();
                *inner = LazyValueRefInner::Resolved {
                    resolved: val.clone(),
                    span,
                };
                Ok(val)
            }
        }
    }

    /// Get value of variable, if resolved
    pub(crate) fn resolved(&self) -> Option<ValueRef> {
        match &*self.inner.borrow() {
            LazyValueRefInner::Resolved { resolved, .. } => Some(resolved.clone()),
            LazyValueRefInner::Pending { .. } => None,
        }
    }

    pub(crate) fn lazy(scope: &Scope, expr: Expr) -> Self {
        Self {
            inner: Rc::new(RefCell::new(LazyValueRefInner::Pending {
                scope_snapshot: Rc::new(scope.snapshot()),
                expr: Some(expr),
            })),
        }
    }

    /// Snapshot self in its current state.  If lazy, then it is _not_ a snapshot, it is just a
    /// reference
    pub fn snapshot(&self) -> Self {
        let inner = match &*self.inner.borrow() {
            LazyValueRefInner::Resolved { resolved, span } => {
                Rc::new(RefCell::new(LazyValueRefInner::Resolved {
                    resolved: resolved.snapshot(),
                    span: *span,
                }))
            }
            LazyValueRefInner::Pending { .. } => self.inner.clone(),
        };
        Self { inner }
    }

    pub fn value_span(&self) -> Span {
        match &*self.inner.borrow() {
            LazyValueRefInner::Resolved { span, .. } => *span,
            LazyValueRefInner::Pending { expr, .. } => expr.as_ref().unwrap().span,
        }
    }
}

impl<V> From<V> for LazyValueRef
where
    V: Value,
{
    fn from(value: V) -> Self {
        ValueRef::new(value).into()
    }
}

impl From<ValueRef> for LazyValueRef {
    fn from(value: ValueRef) -> Self {
        LazyValueRef {
            inner: Rc::new(RefCell::new(LazyValueRefInner::Resolved {
                resolved: value,
                span: Span::empty(),
            })),
        }
    }
}
