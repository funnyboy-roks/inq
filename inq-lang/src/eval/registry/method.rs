use crate::eval::{
    EvalResult,
    registry::{FromVarArgs, VarArgs},
    value::{CallContext, Value, ValueRef},
};

pub trait DynMethod {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef>;
}

impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(&T, V) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        self(this, args).map(Into::into)
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(&T, V) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(this, args).into())
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod
    for fn(CallContext, &T, V) -> EvalResult<Ret>
{
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        self(ctx, this, args).map(Into::into)
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &T, V) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, this, args).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(&T) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        self(this).map(Into::into)
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(&T) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        Ok(self(this).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &T) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        self(ctx, this).map(Into::into)
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &T) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let this = this.value();
        let this = this.unwrap_ref::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, this).into())
    }
}

pub trait Method<T>: DynMethod {}

impl<T: Value, V, R> Method<T> for fn(&T, V) -> R where Self: DynMethod {}
impl<T: Value, V, R> Method<T> for fn(CallContext, &T, V) -> R where Self: DynMethod {}
impl<T: Value, R> Method<T> for fn(CallContext, &T) -> R where Self: DynMethod {}
impl<T: Value, R> Method<T> for fn(&T) -> R where Self: DynMethod {}
