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
    /// Get the index.  This function _must_ return `Ok(None)` if the index is not present
    fn get(&self, index: ValueRef, ctx: CallContext<GetIndexCtx>) -> EvalResult<Option<ValueRef>>;
}

impl<T: Value, I: Value, R: Into<ValueRef>> IndexGetter
    for fn(CallContext<GetIndexCtx>, &T, &I) -> Option<R>
{
    fn get(&self, index: ValueRef, ctx: CallContext<GetIndexCtx>) -> EvalResult<Option<ValueRef>> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        let x = index.value();
        let idx = x.unwrap_ref::<I>();

        Ok(self(ctx, this, idx).map(Into::into))
    }
}
impl<T: Value, I: Value, R: Into<ValueRef>> IndexGetter
    for fn(CallContext<GetIndexCtx>, &T, &I) -> EvalResult<Option<R>>
{
    fn get(&self, index: ValueRef, ctx: CallContext<GetIndexCtx>) -> EvalResult<Option<ValueRef>> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        let x = index.value();
        let idx = x.unwrap_ref::<I>();

        Ok(self(ctx, this, idx)?.map(Into::into))
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

impl<T: Value, I: Value> IndexSetter for fn(CallContext<SetIndexCtx>, &T, &I, ValueRef) -> () {
    fn set(
        &self,
        index: ValueRef,
        value: ValueRef,
        ctx: CallContext<SetIndexCtx>,
    ) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        let x = index.value();
        let idx = x.unwrap_ref::<I>();

        self(ctx, this, idx, value);
        Ok(())
    }
}
impl<T: Value, I: Value> IndexSetter
    for fn(CallContext<SetIndexCtx>, &T, &I, ValueRef) -> EvalResult<()>
{
    fn set(
        &self,
        index: ValueRef,
        value: ValueRef,
        ctx: CallContext<SetIndexCtx>,
    ) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        let x = index.value();
        let idx = x.unwrap_ref::<I>();

        self(ctx, this, idx, value)
    }
}
