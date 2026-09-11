use std::{
    any::TypeId, borrow::Cow, cmp::Ordering, collections::HashMap, fmt::Debug, marker::PhantomData,
    rc::Rc,
};

use crate::eval::{
    DisplayVec, EvalError, EvalResult,
    value::{CallContext, Value, ValueRef},
};

macro_rules! count {
    ($($tt: tt)*) => {
        const { ["",$(stringify!($tt)),*].len() - 1 }
    };
}

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
        ctx: &CallContext,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> EvalResult<T> {
        Err(EvalError::InvalidArgs {
            span: ctx.span,
            got: DisplayVec(self.inner.iter().map(|a| a.type_name_of().into()).collect()),
            expected: DisplayVec(expected.into_iter().map(Into::into).collect()),
        })
    }
}

pub trait FromVarArgs: Sized {
    fn from_varargs(ctx: &CallContext, varargs: VarArgs) -> EvalResult<Self>;
}

impl FromVarArgs for VarArgs {
    fn from_varargs(_ctx: &CallContext, varargs: VarArgs) -> EvalResult<Self> {
        Ok(varargs)
    }
}

impl<V: Value + Clone> FromVarArgs for V {
    fn from_varargs(ctx: &CallContext, mut varargs: VarArgs) -> EvalResult<Self> {
        let expected: [Cow<'static, str>; _] = [V::type_name()];
        if varargs.len() != 1 {
            return varargs.error(ctx, expected);
        }

        let x = varargs.shift().expect("checked above");
        let borrow = x.borrow();
        #[allow(non_snake_case)]
        let Some(v) = borrow.downcast_ref::<V>() else {
            return varargs.error(ctx, expected);
        };

        Ok(v.clone())
    }
}
impl<V: Value + Clone> FromVarArgs for Option<V> {
    fn from_varargs(ctx: &CallContext, mut varargs: VarArgs) -> EvalResult<Self> {
        let expected: [Cow<'static, str>; _] = [V::type_name()];
        if varargs.len() > 1 {
            return varargs.error(ctx, expected);
        }

        let Some(x) = varargs.shift() else {
            return Ok(None);
        };
        let borrow = x.borrow();
        #[allow(non_snake_case)]
        let Some(v) = borrow.downcast_ref::<V>() else {
            return varargs.error(ctx, expected);
        };

        Ok(Some(v.clone()))
    }
}
impl FromVarArgs for ValueRef {
    fn from_varargs(ctx: &CallContext, mut varargs: VarArgs) -> EvalResult<Self> {
        let expected = [Cow::Borrowed("Any")];
        if varargs.len() != 1 {
            return varargs.error(ctx, expected);
        }

        Ok(varargs.shift().expect("checked above"))
    }
}

macro_rules! impl_varargs {
    (# $_: ident => $($tt: tt)*) => { $($tt)* };
    () => {
        impl_varargs!(@);
    };
    ($gen0: ident $($gen: ident)*) => {
        impl_varargs!($($gen)*);
        impl_varargs!(@ $gen0 $($gen)*);

        impl FromVarArgs for (ValueRef, $(impl_varargs!(# $gen => ValueRef),)*) {
            #[allow(unused_mut)]
            fn from_varargs(ctx: &CallContext, mut varargs: VarArgs) -> EvalResult<Self> {
                let expected: [Cow<'static, str>; _] = [$(impl_varargs!(# $gen => Cow::Borrowed("Any")),)*];
                if varargs.len() != count!($gen0 $($gen)*) {
                    return varargs.error(ctx, expected);
                }

                Ok((
                    varargs.shift().expect("checked above"),
                    $(impl_varargs!(# $gen => varargs.shift().expect("checked above")),)*
                ))
            }
        }
    };
    (@ $($gen: ident)*) => {
        impl<$($gen: Value + Clone,)*> FromVarArgs for ($($gen,)*) {
            #[allow(unused_mut)]
            fn from_varargs(ctx: &CallContext, mut varargs: VarArgs) -> EvalResult<Self> {
                let expected: [Cow<'static, str>; _] = [$($gen::type_name()),*];
                if varargs.len() != count!($($gen)*) {
                    return varargs.error(ctx, expected);
                }

                $(
                    let x = varargs.shift().expect("checked above");
                    #[allow(non_snake_case)]
                    let Some($gen) = x.downcast::<$gen>() else {
                        return varargs.error(ctx, expected);
                    };
                )*

                Ok(($($gen,)*))
            }
        }

    };
}
impl_varargs!(V12 V11 V10 V9 V8 V7 V6 V5 V4 V3 V2 V1);

pub trait Function {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef>;
}

impl<V: FromVarArgs, Ret: Into<ValueRef>> Function for fn(CallContext, V) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, args).into())
    }
}
impl<V: FromVarArgs, Ret: Into<ValueRef>> Function for fn(CallContext, V) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let args = V::from_varargs(&ctx, varargs)?;
        self(ctx, args).map(Into::into)
    }
}
impl<V: FromVarArgs, Ret: Into<ValueRef>> Function for fn(V) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(args).into())
    }
}
impl<V: FromVarArgs, Ret: Into<ValueRef>> Function for fn(V) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let args = V::from_varargs(&ctx, varargs)?;
        self(args).map(Into::into)
    }
}

pub trait DynMethod {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef>;
}

impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(&mut T, V) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
        let args = V::from_varargs(&ctx, varargs)?;
        self(this, args).map(Into::into)
    }
}
impl<T: Value, V: FromVarArgs, Ret: Into<ValueRef>> DynMethod for fn(&mut T, V) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
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
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
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
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
        let args = V::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, this, args).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(&mut T) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
        let () = <()>::from_varargs(&ctx, varargs)?;
        self(this).map(Into::into)
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(&mut T) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
        let () = <()>::from_varargs(&ctx, varargs)?;
        Ok(self(this).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &mut T) -> EvalResult<Ret> {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
        let () = <()>::from_varargs(&ctx, varargs)?;
        self(ctx, this).map(Into::into)
    }
}
impl<T: Value, Ret: Into<ValueRef>> DynMethod for fn(CallContext, &mut T) -> Ret {
    fn call(&self, varargs: VarArgs, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let mut this = this.borrow_mut();
        let Some(this) = this.downcast_mut::<T>() else {
            panic!("this type must be checked by caller");
        };
        let () = <()>::from_varargs(&ctx, varargs)?;
        Ok(self(ctx, this).into())
    }
}

pub trait Method<T>: DynMethod {}

impl<T: Value, V, R> Method<T> for fn(&mut T, V) -> R where Self: DynMethod {}
impl<T: Value, V, R> Method<T> for fn(CallContext, &mut T, V) -> R where Self: DynMethod {}
impl<T: Value, R> Method<T> for fn(CallContext, &mut T) -> R where Self: DynMethod {}
impl<T: Value, R> Method<T> for fn(&mut T) -> R where Self: DynMethod {}

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

pub trait CmpFunction {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef) -> Option<Ordering>;
}

#[derive(derive_more::Debug, Clone)]
pub struct FunctionValue(#[debug(skip)] pub Rc<dyn Function>);

impl FunctionValue {
    pub fn new<V, R>(func: fn(CallContext, V) -> R) -> Self
    where
        fn(CallContext, V) -> R: Function + 'static,
    {
        Self(Rc::new(func))
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

impl<T: Value, Rhs: Value> CmpFunction for fn(&T, &Rhs) -> Option<Ordering> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef) -> Option<Ordering> {
        let borrow = lhs.borrow();
        let Some(lhs) = borrow.downcast_ref::<T>() else {
            panic!("type of lhs must be checked by caller");
        };

        let borrow = rhs.borrow();
        let Some(rhs) = borrow.downcast_ref::<Rhs>() else {
            panic!("type of rhs must be checked by caller");
        };

        self(lhs, rhs)
    }
}
impl<T: Value> CmpFunction for fn(&T, &ValueRef) -> Option<Ordering> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef) -> Option<Ordering> {
        let borrow = lhs.borrow();
        let Some(lhs) = borrow.downcast_ref::<T>() else {
            panic!("type of lhs must be checked by caller");
        };

        self(lhs, &rhs)
    }
}

#[derive(Clone)]
pub(crate) struct Field {
    pub(crate) getter: Rc<dyn Getter>,
    pub(crate) setter: Option<Rc<dyn Setter>>,
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

#[derive(Clone)]
pub(crate) struct Indexer {
    pub(crate) getter: Rc<dyn IndexGetter>,
    pub(crate) setter: Option<Rc<dyn IndexSetter>>,
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

#[derive(derive_more::Debug)]
pub(crate) struct AnyRegistry {
    #[debug("{:?}", methods.keys().collect::<Vec<_>>())]
    methods: HashMap<&'static str, Rc<dyn DynMethod>>,
    fields: HashMap<&'static str, Field>,
    /// (op, none) -> lhs <op> Any Value
    /// (op, Some(rhs)) -> lhs <op> rhs
    #[debug("{:?}", bin_ops.keys().collect::<Vec<_>>())]
    bin_ops: HashMap<(BinOp, Option<TypeId>), Rc<dyn BinOpFunction>>,
    #[debug("{:?}", unary_ops.keys().collect::<Vec<_>>())]
    unary_ops: HashMap<UnaryOp, Rc<dyn UnaryOpFunction>>,
    #[debug("{:?}", cmps.keys().collect::<Vec<_>>())]
    cmps: HashMap<Option<TypeId>, Rc<dyn CmpFunction>>,
    #[debug("{}", if call.is_some() { "Some(..)" } else { "None" })]
    pub(crate) call: Option<Rc<dyn DynMethod>>,
    indexers: HashMap<TypeId, Indexer>,
}

impl AnyRegistry {
    pub(crate) fn get_method(&self, inner: &'_ str) -> Option<Rc<dyn DynMethod>> {
        self.methods.get(inner).cloned()
    }

    pub(crate) fn get_field(&self, inner: &'_ str) -> Option<&Field> {
        self.fields.get(inner)
    }

    pub(crate) fn get_bin_op(&self, op: BinOp, rhs_tid: TypeId) -> Option<Rc<dyn BinOpFunction>> {
        self.bin_ops
            .get(&(op, Some(rhs_tid)))
            .or_else(|| self.bin_ops.get(&(op, None)))
            .cloned()
    }

    pub(crate) fn get_unary_op(&self, op: UnaryOp) -> Option<Rc<dyn UnaryOpFunction>> {
        self.unary_ops.get(&op).cloned()
    }

    pub(crate) fn get_index(&self, type_id: TypeId) -> Option<Indexer> {
        self.indexers.get(&type_id).cloned()
    }

    pub(crate) fn get_cmp(&self, tid: Option<TypeId>) -> Option<Rc<dyn CmpFunction>> {
        self.cmps.get(&tid).cloned()
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
                cmps: Default::default(),
            },
            _p: PhantomData,
        }
    }

    pub fn register_method<F>(&mut self, name: &'static str, func: F)
    where
        F: Method<T> + 'static,
    {
        self.inner.methods.insert(name, Rc::new(func));
    }

    pub fn register_call<F>(&mut self, func: F)
    where
        F: Method<T> + 'static,
    {
        self.inner.call = Some(Rc::new(func));
    }

    pub fn register_field_get<Ret>(&mut self, name: &'static str, func: fn(&mut T) -> Ret)
    where
        fn(&mut T) -> Ret: Method<T> + Getter + 'static,
    {
        self.inner.fields.insert(
            name,
            Field {
                getter: Rc::new(func),
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
                getter: Rc::new(getter),
                setter: Some(Rc::new(setter)),
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
                getter: Rc::new(func),
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
                getter: Rc::new(getter),
                setter: Some(Rc::new(setter)),
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
        self.inner.bin_ops.insert((op, tid), Rc::new(func));
    }

    pub fn register_unary_op<Ret: 'static>(&mut self, op: UnaryOp, func: fn(CallContext, &T) -> Ret)
    where
        fn(CallContext, &T) -> Ret: UnaryOpFunction,
    {
        self.inner.unary_ops.insert(op, Rc::new(func));
    }

    pub fn register_cmp<Rhs: 'static>(&mut self, func: fn(&T, &Rhs) -> Option<Ordering>)
    where
        fn(&T, &Rhs) -> Option<Ordering>: CmpFunction,
    {
        let tid = if TypeId::of::<Rhs>() == TypeId::of::<ValueRef>() {
            None
        } else {
            Some(TypeId::of::<Rhs>())
        };
        self.inner.cmps.insert(tid, Rc::new(func));
    }

    pub(crate) fn erase(self) -> (TypeId, AnyRegistry) {
        (TypeId::of::<T>(), self.inner)
    }
}
