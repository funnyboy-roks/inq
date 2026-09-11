use std::{cell::RefCell, rc::Rc};

use crate::lang::{
    eval::{
        EvalResult, Scope,
        value::{Value, ValueRef},
    },
    expr::Expr,
};

#[derive(Clone, Debug)]
enum LazyValueRefInner {
    Resolved(ValueRef),
    Pending {
        /// Snapshot of the scope at the time the variable was set
        scope_snapshot: Rc<Scope>,
        /// Option just so it can be taken
        expr: Option<Expr>,
    },
}

#[derive(derive_more::Debug, Clone)]
#[debug("{:?}", inner.borrow())]
pub(crate) struct LazyValueRef {
    inner: Rc<RefCell<LazyValueRefInner>>,
}

impl LazyValueRef {
    pub fn get(&self) -> EvalResult<ValueRef> {
        let mut guard = self.inner.borrow_mut();
        match &mut *guard {
            LazyValueRefInner::Resolved(v) => Ok(v.clone()),
            LazyValueRefInner::Pending {
                scope_snapshot,
                expr,
            } => {
                let expr = expr.take().expect(
                    "This take sets inner to resolved and this option is not taken anywhere else",
                );

                let val = scope_snapshot.eval(expr)?;
                drop(guard);
                let mut inner = self.inner.borrow_mut();
                *inner = LazyValueRefInner::Resolved(val.clone());
                Ok(val)
            }
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
            inner: Rc::new(RefCell::new(LazyValueRefInner::Resolved(value))),
        }
    }
}
