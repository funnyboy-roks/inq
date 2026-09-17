use crate::eval::{
    EvalResult,
    registry::{FromVarArgs, VarArgs},
    value::{CallContext, Value, ValueRef},
};

pub trait DynMethod {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef>;
}

impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(&mut T, V) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        self(this, args).map(Into::into)
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(&mut T, V) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(this, args).into())
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod
    for fn(CallContext, &mut T, V) -> EvalResult<Ret>
{
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        self(ctx, this, args).map(Into::into)
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod
    for fn(CallContext, &mut T, V) -> Ret
{
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, this, args).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(&mut T) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        self(this).map(Into::into)
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(&mut T) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        Ok(self(this).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &mut T) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        self(ctx, this).map(Into::into)
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &mut T) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let this = this.unwrap_mut::<T>();
        let () = <()>::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, this).into())
    }
}

pub trait Method<T>: DynMethod {}

impl<T: Value, V, R> Method<T> for fn(&mut T, V) -> R where Self: DynMethod {}
impl<T: Value, V, R> Method<T> for fn(CallContext, &mut T, V) -> R where Self: DynMethod {}
impl<T: Value, R> Method<T> for fn(CallContext, &mut T) -> R where Self: DynMethod {}
impl<T: Value, R> Method<T> for fn(&mut T) -> R where Self: DynMethod {}
