use crate::{
    eval::{
        EvalError, EvalResult,
        value::{CallContext, Value, ValueRef},
    },
    parse::Ident,
};

pub trait Getter {
    fn get(&self, ctx: CallContext) -> EvalResult<ValueRef>;
}

pub trait Setter {
    fn set(&self, value: ValueRef, ctx: CallContext) -> EvalResult<()>;
}

impl<T: Value, R: Into<ValueRef>> Getter for fn(CallContext, &T) -> R {
    fn get(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        Ok(self(ctx, this).into())
    }
}
impl<T: Value, R: Into<ValueRef>> Getter for fn(CallContext, &T) -> EvalResult<R> {
    fn get(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        Ok(self(ctx, this)?.into())
    }
}

impl<T: Value> Setter for fn(CallContext, &T, ValueRef) -> EvalResult<()> {
    fn set(&self, value: ValueRef, ctx: CallContext) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();
        self(ctx, this, value)
    }
}
impl<T: Value> Setter for fn(CallContext, &T, ValueRef) {
    fn set(&self, value: ValueRef, ctx: CallContext) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();
        self(ctx, this, value);
        Ok(())
    }
}

pub trait FieldGetFallback {
    fn get(&self, ctx: CallContext, field: Ident) -> EvalResult<ValueRef>;
}

pub trait FieldSetFallback {
    fn set(&self, ctx: CallContext, field: Ident, value: ValueRef) -> EvalResult<()>;
}

impl<T: Value, R: Into<ValueRef>> FieldGetFallback for fn(CallContext, &T, Ident) -> R {
    fn get(&self, ctx: CallContext, field: Ident) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        Ok(self(ctx, this, field).into())
    }
}
impl<T: Value, R: Into<ValueRef>> FieldGetFallback for fn(CallContext, &T, Ident) -> EvalResult<R> {
    fn get(&self, ctx: CallContext, field: Ident) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();

        Ok(self(ctx, this, field)?.into())
    }
}

impl<T: Value> FieldSetFallback for fn(CallContext, &T, Ident, ValueRef) -> EvalResult<()> {
    fn set(&self, ctx: CallContext, field: Ident, value: ValueRef) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();
        self(ctx, this, field, value)
    }
}
impl<T: Value> FieldSetFallback for fn(CallContext, &T, Ident, ValueRef) {
    fn set(&self, ctx: CallContext, field: Ident, value: ValueRef) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let x = this.value();
        let this = x.unwrap_ref::<T>();
        self(ctx, this, field, value);
        Ok(())
    }
}

pub(super) struct UnknownField;
impl FieldGetFallback for UnknownField {
    fn get(&self, ctx: CallContext, field: Ident) -> EvalResult<ValueRef> {
        Err(EvalError::UnknownField {
            ty: ctx.self_ref.type_name_of().into(),
            field: field.clone(),
        })
    }
}
impl FieldSetFallback for UnknownField {
    fn set(&self, ctx: CallContext, field: Ident, _value: ValueRef) -> EvalResult<()> {
        Err(EvalError::UnknownField {
            ty: ctx.self_ref.type_name_of().into(),
            field: field.clone(),
        })
    }
}
