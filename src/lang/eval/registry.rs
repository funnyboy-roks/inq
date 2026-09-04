use std::{any::TypeId, borrow::Cow, collections::HashMap, fmt::Debug, marker::PhantomData};

use crate::lang::eval::{
    DisplayVec, EvalError, EvalResult,
    value::{CallContext, Value, ValueRef},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum BinOp {
    #[display("add")]
    Add,
    #[display("subtract")]
    Sub,
    #[display("multiply")]
    Mul,
    #[display("divide")]
    Div,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum PrefixOp {
    #[display("negate")]
    Neg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum PostfixOp {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum UnaryOp {
    #[display("{_0}")]
    Prefix(PrefixOp),
    #[display("{_0}")]
    Postfix(PostfixOp),
}

pub struct VarArgs {
    pub inner: Vec<ValueRef>,
    index: usize,
}

impl VarArgs {
    pub fn empty() -> Self {
        Self {
            inner: Default::default(),
            index: 0,
        }
    }

    pub fn new(inner: impl Into<Vec<ValueRef>>) -> Self {
        Self {
            inner: inner.into(),
            index: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len().saturating_sub(self.index)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn shift(&mut self) -> Option<ValueRef> {
        let r = self.inner.get(self.index)?;
        self.index += 1;
        Some(r.clone())
    }

    fn error<T>(
        &self,
        ctx: CallContext,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> EvalResult<T> {
        Err(EvalError::InvalidArgs {
            span: ctx.span,
            got: DisplayVec(self.inner.iter().map(|a| a.type_name_of().into()).collect()),
            expected: DisplayVec(expected.into_iter().map(Into::into).collect()),
        })
    }
}

pub trait Function {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef>;
}

pub trait DynMethod {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef>;
}

pub trait Method<T>: DynMethod {}

pub trait Getter: DynMethod {
    fn get(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        self.call(VarArgs::empty(), ctx)
    }
}

pub trait Setter {
    fn set(&self, value: ValueRef, ctx: CallContext) -> EvalResult<()>;
}

pub trait IndexGetter {
    fn get(&self, index: ValueRef, ctx: CallContext) -> EvalResult<ValueRef>;
}

impl<T: Value, I: Value, R: Into<ValueRef>> IndexGetter for fn(CallContext, &T, &I) -> R {
    fn get(&self, index: ValueRef, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.borrow();
        let Some(this) = x.downcast_ref::<T>() else {
            panic!("this type should be checked by caller");
        };

        let x = index.borrow();
        let Some(idx) = x.downcast_ref::<I>() else {
            panic!("this type should be checked by caller");
        };

        Ok(self(ctx, this, idx).into())
    }
}
impl<T: Value, I: Value, R: Into<ValueRef>> IndexGetter
    for fn(CallContext, &T, &I) -> EvalResult<R>
{
    fn get(&self, index: ValueRef, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let x = this.borrow();
        let Some(this) = x.downcast_ref::<T>() else {
            panic!("this type should be checked by caller");
        };

        let x = index.borrow();
        let Some(idx) = x.downcast_ref::<I>() else {
            panic!("this type should be checked by caller");
        };

        Ok(self(ctx, this, idx)?.into())
    }
}

pub trait IndexSetter {
    fn set(&self, index: ValueRef, value: ValueRef, ctx: CallContext) -> EvalResult<()>;
}

impl<T: Value, I: Value> IndexSetter for fn(CallContext, &mut T, &I, ValueRef) -> () {
    fn set(&self, index: ValueRef, value: ValueRef, ctx: CallContext) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };

        let x = index.borrow();
        let Some(idx) = x.downcast_ref::<I>() else {
            panic!("this type should be checked by caller");
        };

        self(ctx, this, idx, value);
        Ok(())
    }
}
impl<T: Value, I: Value> IndexSetter for fn(CallContext, &mut T, &I, ValueRef) -> EvalResult<()> {
    fn set(&self, index: ValueRef, value: ValueRef, ctx: CallContext) -> EvalResult<()> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };

        let x = index.borrow();
        let Some(idx) = x.downcast_ref::<I>() else {
            panic!("this type should be checked by caller");
        };

        self(ctx, this, idx, value)
    }
}

impl<T: Value> Setter for fn(&mut T, ValueRef) -> EvalResult<()> {
    fn set(&self, value: ValueRef, ctx: CallContext) -> EvalResult<()> {
        let mut x = ctx.self_ref.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };
        self(this, value)
    }
}
impl<T: Value> Setter for fn(&mut T, ValueRef) {
    fn set(&self, value: ValueRef, ctx: CallContext) -> EvalResult<()> {
        let mut x = ctx.self_ref.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };
        self(this, value);
        Ok(())
    }
}

pub trait BinOpFunction {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, ctx: CallContext) -> EvalResult<ValueRef>;
}

pub trait UnaryOpFunction {
    fn apply(&self, ctx: CallContext) -> EvalResult<ValueRef>;
}

macro_rules! count {
    ($($tt: tt)*) => {
        const { ["",$(stringify!($tt)),*].len() - 1 }
    };
}

macro_rules! impl_function {
    () => {
        impl_function!(@);
    };
    ($gen0: ident $($gen: ident)*) => {
        impl_function!($($gen)*);
        impl_function!(@ $gen0 $($gen)*);
    };
    (@ $($gen: ident)*) => {
        impl<$($gen: Value,)* R: Into<ValueRef>> Function for fn($(&$gen),*) -> R {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(self($($gen),*).into())
            }
        }
        impl<$($gen: Value,)* R: Into<ValueRef>> Function for fn($(&$gen),*) -> EvalResult<R> {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                self($($gen),*).map(Into::into)
            }
        }

        impl<$($gen: Value,)* R: Into<ValueRef>> Function for fn(CallContext, $(&$gen),*) -> R {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(self(ctx, $($gen),*).into())
            }
        }
        impl<$($gen: Value,)* R: Into<ValueRef>> Function for fn(CallContext, $(&$gen),*) -> EvalResult<R> {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                self(ctx, $($gen),*).map(Into::into)
            }
        }

        impl<T: Value, $($gen: Value,)* R: Into<ValueRef>> DynMethod for fn(&mut T, $(&$gen),*) -> R {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let this = ctx.self_ref.clone();
                let mut x = this.borrow_mut();
                let Some(this) = x.downcast_mut::<T>() else {
                    panic!("this type should be checked by caller");
                };

                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(self(this, $($gen),*).into())
            }
        }
        impl<T: Value, $($gen: Value,)* R: Into<ValueRef>> DynMethod for fn(&mut T, $(&$gen),*) -> EvalResult<R> {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let this = ctx.self_ref.clone();
                let mut x = this.borrow_mut();
                let Some(this) = x.downcast_mut::<T>() else {
                    panic!("this type should be checked by caller");
                };

                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(self(this, $($gen),*)?.into())
            }
        }

        impl<T: Value, $($gen: Value,)* R: Into<ValueRef>> DynMethod for fn(CallContext, &mut T, $(&$gen),*) -> R {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let this = ctx.self_ref.clone();
                let mut x = this.borrow_mut();
                let Some(this) = x.downcast_mut::<T>() else {
                    panic!("this type should be checked by caller");
                };

                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(self(ctx, this, $($gen),*).into())
            }
        }
        impl<T: Value, $($gen: Value,)* R: Into<ValueRef>> DynMethod for fn(CallContext, &mut T, $(&$gen),*) -> EvalResult<R> {
            #[allow(unused_mut)]
            fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
                let this = ctx.self_ref.clone();
                let mut x = this.borrow_mut();
                let Some(this) = x.downcast_mut::<T>() else {
                    panic!("this type should be checked by caller");
                };

                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    let borrow = x.borrow();
                    #[allow(non_snake_case)]
                    let Some($gen) = borrow.downcast_ref::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(self(ctx, this, $($gen),*)?.into())
            }
        }

        impl<T, $($gen,)* R> Method<T> for fn(&mut T, $(&$gen),*) -> R where Self: DynMethod {}
        impl<T, $($gen,)* R> Method<T> for fn(CallContext, &mut T, $(&$gen),*) -> R where Self: DynMethod {}
    }
}

impl_function!(V12 V11 V10 V9 V8 V7 V6 V5 V4 V3 V2 V1);

impl<R: Into<ValueRef>> Function for fn(VarArgs) -> R {
    fn call(&self, varargs: VarArgs, _: CallContext) -> EvalResult<ValueRef> {
        Ok(self(varargs).into())
    }
}
impl<R: Into<ValueRef>> Function for fn(VarArgs) -> EvalResult<R> {
    fn call(&self, varargs: VarArgs, _: CallContext) -> EvalResult<ValueRef> {
        Ok(self(varargs)?.into())
    }
}

impl<R: Into<ValueRef>> Function for fn(CallContext, VarArgs) -> R {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        Ok(self(ctx, varargs).into())
    }
}
impl<R: Into<ValueRef>> Function for fn(CallContext, VarArgs) -> EvalResult<R> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        Ok(self(ctx, varargs)?.into())
    }
}

impl<R: Into<ValueRef>> Function for fn(CallContext, ValueRef) -> R {
    fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        if varargs.len() != 1 {
            return varargs.error(ctx, ["Any"]);
        }
        let val = varargs.shift().expect("checked above");
        Ok(self(ctx, val).into())
    }
}
impl<R: Into<ValueRef>> Function for fn(CallContext, ValueRef) -> EvalResult<R> {
    fn call(&self, mut varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        if varargs.len() != 1 {
            return varargs.error(ctx, ["Any"]);
        }
        let val = varargs.shift().expect("checked above");
        Ok(self(ctx, val)?.into())
    }
}

#[derive(derive_more::Debug)]
pub(crate) struct FunctionValue(#[debug(skip)] pub Box<dyn Function>);

impl FunctionValue {
    pub fn new<F: Function + 'static>(func: F) -> Self {
        Self(Box::new(func))
    }
}

impl Value for FunctionValue {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized,
    {
        "Function".into()
    }

    fn type_name_of(&self) -> Cow<'static, str> {
        "Function".into()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str("<native function>");
    }

    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_call::<fn(_, &mut _, VarArgs) -> _>(|ctx, f, args| f.0.call(args, ctx));
    }
}

impl<T: Value, R: Into<ValueRef>> DynMethod for fn(&mut T, VarArgs) -> EvalResult<R> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };

        self(this, varargs).map(Into::into)
    }
}
impl<T: Value, R: Into<ValueRef>> DynMethod for fn(&mut T, VarArgs) -> R {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };

        Ok(self(this, varargs).into())
    }
}

impl<T: Value, R: Into<ValueRef>> DynMethod for fn(CallContext, &mut T, VarArgs) -> EvalResult<R> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };

        self(ctx, this, varargs).map(Into::into)
    }
}
impl<T: Value, R: Into<ValueRef>> DynMethod for fn(CallContext, &mut T, VarArgs) -> R {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut x = this.borrow_mut();
        let Some(this) = x.downcast_mut::<T>() else {
            panic!("this type should be checked by caller");
        };

        Ok(self(ctx, this, varargs).into())
    }
}

impl<T, R> Method<T> for fn(CallContext, &mut T, VarArgs) -> R where Self: DynMethod {}
impl<T, R> Method<T> for fn(&mut T, VarArgs) -> R where Self: DynMethod {}
impl<T, R> Getter for fn(&mut T) -> R where Self: Method<T> {}

impl<L: Value, R: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &R) -> Ret {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let Some(lhs) = borrow.downcast_ref::<L>() else {
            panic!("lhs must be checked by caller");
        };
        let borrow = rhs.borrow();
        let Some(rhs) = borrow.downcast_ref::<R>() else {
            panic!("rhs must be checked by caller");
        };

        Ok(self(lhs, rhs).into())
    }
}
impl<L: Value, R: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &R) -> EvalResult<Ret> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let Some(lhs) = borrow.downcast_ref::<L>() else {
            panic!("lhs must be checked by caller");
        };
        let borrow = rhs.borrow();
        let Some(rhs) = borrow.downcast_ref::<R>() else {
            panic!("rhs must be checked by caller");
        };

        Ok(self(lhs, rhs)?.into())
    }
}
impl<L: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &ValueRef) -> Ret {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let Some(lhs) = borrow.downcast_ref::<L>() else {
            panic!("lhs must be checked by caller");
        };

        Ok(self(lhs, &rhs).into())
    }
}
impl<L: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &ValueRef) -> EvalResult<Ret> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let Some(lhs) = borrow.downcast_ref::<L>() else {
            panic!("lhs must be checked by caller");
        };

        Ok(self(lhs, &rhs)?.into())
    }
}

impl<T: Value, Ret: Into<ValueRef>> UnaryOpFunction for fn(CallContext, &T) -> Ret {
    fn apply(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let borrow = this.borrow();
        let Some(this) = borrow.downcast_ref::<T>() else {
            panic!("this must be checked by caller");
        };

        Ok(self(ctx, this).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> UnaryOpFunction for fn(CallContext, &T) -> EvalResult<Ret> {
    fn apply(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let borrow = this.borrow();
        let Some(this) = borrow.downcast_ref::<T>() else {
            panic!("this must be checked by caller");
        };

        Ok(self(ctx, this)?.into())
    }
}

pub(crate) struct Field {
    pub(crate) getter: Box<dyn Getter>,
    pub(crate) setter: Option<Box<dyn Setter>>,
}

impl Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("getter", &std::fmt::from_fn(|f| write!(f, "..")))
            .field(
                "setter",
                &std::fmt::from_fn(|f| {
                    if self.setter.is_some() {
                        write!(f, "Some(..)")
                    } else {
                        write!(f, "None")
                    }
                }),
            )
            .finish()
    }
}

pub(crate) struct Indexer {
    pub(crate) getter: Box<dyn IndexGetter>,
    pub(crate) setter: Option<Box<dyn IndexSetter>>,
}

impl Debug for Indexer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Indexer")
            .field("getter", &std::fmt::from_fn(|f| write!(f, "..")))
            .field(
                "setter",
                &std::fmt::from_fn(|f| {
                    if self.setter.is_some() {
                        write!(f, "Some(..)")
                    } else {
                        write!(f, "None")
                    }
                }),
            )
            .finish()
    }
}

pub struct Registry<T> {
    inner: AnyRegistry,
    _p: PhantomData<T>,
}

pub(crate) struct AnyRegistry {
    methods: HashMap<&'static str, Box<dyn DynMethod>>,
    fields: HashMap<&'static str, Field>,
    /// (op, none) -> lhs <op> Any Value
    /// (op, Some(rhs)) -> lhs <op> rhs
    bin_ops: HashMap<(BinOp, Option<TypeId>), Box<dyn BinOpFunction>>,
    unary_ops: HashMap<UnaryOp, Box<dyn UnaryOpFunction>>,
    pub(crate) call: Option<Box<dyn DynMethod>>,
    indexers: HashMap<TypeId, Indexer>,
}
impl AnyRegistry {
    pub(crate) fn get_method(&self, inner: &'_ str) -> Option<&dyn DynMethod> {
        self.methods.get(inner).map(|f| &**f)
    }

    pub(crate) fn get_field(&self, inner: &'_ str) -> Option<&Field> {
        self.fields.get(inner)
    }

    pub(crate) fn get_bin_op(&self, op: BinOp, rhs_tid: TypeId) -> Option<&dyn BinOpFunction> {
        self.bin_ops
            .get(&(op, Some(rhs_tid)))
            .or_else(|| self.bin_ops.get(&(op, None)))
            .map(|f| &**f)
    }

    pub(crate) fn get_unary_op(&self, op: UnaryOp) -> Option<&dyn UnaryOpFunction> {
        self.unary_ops.get(&op).map(|v| &**v)
    }

    pub(crate) fn get_index(&self, type_id: TypeId) -> Option<&Indexer> {
        self.indexers.get(&type_id)
    }
}

impl Debug for AnyRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyRegistry")
            .field(
                "methods",
                &std::fmt::from_fn(|fmt| fmt.debug_set().entries(self.methods.keys()).finish()),
            )
            .field("fields", &self.fields)
            .field(
                "bin_ops",
                &std::fmt::from_fn(|fmt| fmt.debug_set().entries(self.bin_ops.keys()).finish()),
            )
            .field(
                "unary_ops",
                &std::fmt::from_fn(|fmt| fmt.debug_set().entries(self.bin_ops.keys()).finish()),
            )
            .field(
                "call",
                &std::fmt::from_fn(|fmt| {
                    if self.call.is_some() {
                        write!(fmt, "Some(..)")
                    } else {
                        write!(fmt, "None")
                    }
                }),
            )
            .field("indexers", &self.indexers)
            .finish_non_exhaustive()
    }
}

impl<T: Value> Registry<T> {
    pub(crate) fn new() -> Self {
        Self {
            inner: AnyRegistry {
                methods: Default::default(),
                fields: Default::default(),
                bin_ops: Default::default(),
                unary_ops: Default::default(),
                call: Default::default(),
                indexers: Default::default(),
            },
            _p: PhantomData,
        }
    }

    pub fn register_method<F>(&mut self, name: &'static str, func: F)
    where
        F: Method<T> + 'static,
    {
        self.inner.methods.insert(name, Box::new(func));
    }

    pub fn register_call<F>(&mut self, func: F)
    where
        F: Method<T> + 'static,
    {
        self.inner.call = Some(Box::new(func));
    }

    pub fn register_field_get<Ret>(&mut self, name: &'static str, func: fn(&mut T) -> Ret)
    where
        fn(&mut T) -> Ret: Method<T> + Getter + 'static,
    {
        self.inner.fields.insert(
            name,
            Field {
                getter: Box::new(func),
                setter: None,
            },
        );
    }

    pub fn register_field_get_set<Ret: 'static>(
        &mut self,
        name: &'static str,
        getter: fn(&mut T) -> Ret,
        setter: fn(&mut T, ValueRef),
    ) where
        fn(&mut T) -> Ret: Method<T> + Getter + 'static,
        fn(&mut T, ValueRef): Setter + 'static,
    {
        self.inner.fields.insert(
            name,
            Field {
                getter: Box::new(getter),
                setter: Some(Box::new(setter)),
            },
        );
    }

    pub fn register_index_get<IndexType: Value + 'static, Ret>(
        &mut self,
        func: fn(CallContext, &T, &IndexType) -> Ret,
    ) where
        fn(CallContext, &T, &IndexType) -> Ret: IndexGetter + 'static,
    {
        self.inner.indexers.insert(
            TypeId::of::<IndexType>(),
            Indexer {
                getter: Box::new(func),
                setter: None,
            },
        );
    }

    pub fn register_index_get_set<IndexType, Ret, SR>(
        &mut self,
        getter: fn(CallContext, &T, &IndexType) -> Ret,
        setter: fn(CallContext, &mut T, &IndexType, ValueRef) -> SR,
    ) where
        fn(CallContext, &T, &IndexType) -> Ret: IndexGetter + 'static,
        fn(CallContext, &mut T, &IndexType, ValueRef) -> SR: IndexSetter + 'static,
    {
        self.inner.indexers.insert(
            TypeId::of::<IndexType>(),
            Indexer {
                getter: Box::new(getter),
                setter: Some(Box::new(setter)),
            },
        );
    }

    pub fn register_bin_op<Rhs: 'static, Ret: 'static>(
        &mut self,
        op: BinOp,
        func: fn(&T, &Rhs) -> Ret,
    ) where
        fn(&T, &Rhs) -> Ret: BinOpFunction,
    {
        let tid = if TypeId::of::<Rhs>() == TypeId::of::<ValueRef>() {
            None
        } else {
            Some(TypeId::of::<Rhs>())
        };
        self.inner.bin_ops.insert((op, tid), Box::new(func));
    }

    pub fn register_unary_op<Ret: 'static>(&mut self, op: UnaryOp, func: fn(CallContext, &T) -> Ret)
    where
        fn(CallContext, &T) -> Ret: UnaryOpFunction,
    {
        self.inner.unary_ops.insert(op, Box::new(func));
    }

    pub(crate) fn erase(self) -> (TypeId, AnyRegistry) {
        (TypeId::of::<T>(), self.inner)
    }
}
