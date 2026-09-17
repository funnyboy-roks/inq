use crate::{
    Span,
    eval::{
        EvalResult,
        value::{CallContext, Value, ValueRef},
    },
};

#[derive(Debug, Clone, Copy)]
pub struct GetIndexCtx {
    pub index_span: Span,
}

pub trait IndexGetter {
    fn get(&self, index: ValueRef, ctx: CallContext<GetIndexCtx>) -> EvalResult<ValueRef>;
}

impl<T: Value, I: Value, R: Into<ValueRef>> IndexGetter
    for fn(CallContext<GetIndexCtx>, &T, &I) -> R
{
    fn get(&self, index: ValueRef, ctx: CallContext<GetIndexCtx>) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.borrow();
        let this = x.unwrap_ref::<T>();

        let x = index.borrow();
        let idx = x.unwrap_ref::<I>();

        Ok(self(ctx, this, idx).into())
    }
}
impl<T: Value, I: Value, R: Into<ValueRef>> IndexGetter
    for fn(CallContext<GetIndexCtx>, &T, &I) -> EvalResult<R>
{
    fn get(&self, index: ValueRef, ctx: CallContext<GetIndexCtx>) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.borrow();
        let this = x.unwrap_ref::<T>();

        let x = index.borrow();
        let idx = x.unwrap_ref::<I>();

        Ok(self(ctx, this, idx)?.into())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SetIndexCtx {
    pub rhs_span: Span,
    pub index_span: Span,
}

pub trait IndexSetter {
    fn set(
        &self,
        index: ValueRef,
        value: ValueRef,
        ctx: CallContext<SetIndexCtx>,
    ) -> EvalResult<()>;
}

impl<T: Value, I: Value> IndexSetter for fn(CallContext<SetIndexCtx>, &mut T, &I, ValueRef) -> () {
    fn set(
        &self,
        index: ValueRef,
        value: ValueRef,
        ctx: CallContext<SetIndexCtx>,
    ) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let this = x.unwrap_mut::<T>();

        let x = index.borrow();
        let idx = x.unwrap_ref::<I>();

        self(ctx, this, idx, value);
        Ok(())
    }
}
impl<T: Value, I: Value> IndexSetter
    for fn(CallContext<SetIndexCtx>, &mut T, &I, ValueRef) -> EvalResult<()>
{
    fn set(
        &self,
        index: ValueRef,
        value: ValueRef,
        ctx: CallContext<SetIndexCtx>,
    ) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let this = x.unwrap_mut::<T>();

        let x = index.borrow();
        let idx = x.unwrap_ref::<I>();

        self(ctx, this, idx, value)
    }
}
