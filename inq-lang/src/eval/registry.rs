use std::{
    any::TypeId, borrow::Cow, cell::RefCell, cmp::Ordering, collections::HashMap, fmt::Debug,
    marker::PhantomData, rc::Rc,
};

use crate::{
    IStr,
    eval::{
        Engine, EvalError, EvalResult,
        value::{CallContext, Value, ValueRef},
    },
    parse::Ident,
};

macro_rules! count {
    ($($tt: tt)*) => {
        const { ["",$(stringify!($tt)),*].len() - 1 }
    };
}

mod field;
pub use field::*;
mod index;
pub use index::*;
mod method;
pub use method::*;

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
            got: self.inner.iter().map(|a| a.type_name_of().into()).collect(),
            expected: expected.into_iter().map(Into::into).collect(),
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
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "<native function>")
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
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

impl<L: Value, R: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &R) -> Ret {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let lhs = borrow.unwrap_ref::<L>();
        let borrow = rhs.borrow();
        let rhs = borrow.unwrap_ref::<R>();

        Ok(self(lhs, rhs).into())
    }
}
impl<L: Value, R: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &R) -> EvalResult<Ret> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let lhs = borrow.unwrap_ref::<L>();
        let borrow = rhs.borrow();
        let rhs = borrow.unwrap_ref::<R>();

        Ok(self(lhs, rhs)?.into())
    }
}
impl<L: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &ValueRef) -> Ret {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let lhs = borrow.unwrap_ref::<L>();

        Ok(self(lhs, &rhs).into())
    }
}
impl<L: Value, Ret: Into<ValueRef>> BinOpFunction for fn(&L, &ValueRef) -> EvalResult<Ret> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef, _ctx: CallContext) -> EvalResult<ValueRef> {
        let borrow = lhs.borrow();
        let lhs = borrow.unwrap_ref::<L>();

        Ok(self(lhs, &rhs)?.into())
    }
}

impl<T: Value, Ret: Into<ValueRef>> UnaryOpFunction for fn(CallContext, &T) -> Ret {
    fn apply(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let borrow = this.borrow();
        let this = borrow.unwrap_ref::<T>();

        Ok(self(ctx, this).into())
    }
}
impl<T: Value, Ret: Into<ValueRef>> UnaryOpFunction for fn(CallContext, &T) -> EvalResult<Ret> {
    fn apply(&self, ctx: CallContext) -> EvalResult<ValueRef> {
        let this = ctx.self_ref.clone();
        let borrow = this.borrow();
        let this = borrow.unwrap_ref::<T>();

        Ok(self(ctx, this)?.into())
    }
}

impl<T: Value, Rhs: Value> CmpFunction for fn(&T, &Rhs) -> Option<Ordering> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef) -> Option<Ordering> {
        let borrow = lhs.borrow();
        let lhs = borrow.unwrap_ref::<T>();

        let borrow = rhs.borrow();
        let rhs = borrow.unwrap_ref::<Rhs>();

        self(lhs, rhs)
    }
}
impl<T: Value> CmpFunction for fn(&T, &ValueRef) -> Option<Ordering> {
    fn apply(&self, lhs: ValueRef, rhs: ValueRef) -> Option<Ordering> {
        let borrow = lhs.borrow();
        let lhs = borrow.unwrap_ref::<T>();

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

fn unknown_method(ctx: CallContext, method: Ident, _: VarArgs) -> EvalResult<ValueRef> {
    Err(EvalError::UnknownMethod {
        ty: ctx.self_ref.type_name_of().into(),
        method,
    })
}

pub struct Registry<T> {
    inner: AnyRegistry,
    _p: PhantomData<T>,
}

#[derive(derive_more::Debug)]
pub(crate) struct AnyRegistry {
    pub(crate) name: IStr,
    pub(crate) type_id: TypeId,
    #[debug("..")]
    engine: Rc<Engine>,
    #[debug("{:?}", methods.keys().collect::<Vec<_>>())]
    methods: HashMap<&'static str, Rc<dyn DynMethod>>,
    method_fallback: fn(CallContext, Ident, VarArgs) -> EvalResult<ValueRef>,
    #[debug("{:?}", methods.keys().collect::<Vec<_>>())]
    static_methods: HashMap<&'static str, Rc<dyn Function>>,
    static_method_fallback: fn(CallContext, Ident, VarArgs) -> Result<ValueRef, EvalError>,
    fields: HashMap<&'static str, Field>,
    #[debug("{}", if call.is_some() { "Some(..)" } else { "None" })]
    pub(crate) field_get_fallback: Rc<dyn FieldGetFallback>,
    #[debug("{}", if call.is_some() { "Some(..)" } else { "None" })]
    pub(crate) field_set_fallback: Rc<dyn FieldSetFallback>,
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
    pub(crate) fn for_same_type(&self, other: &AnyRegistry) -> bool {
        self.type_id == other.type_id
    }

    pub(crate) fn call_method(
        &self,
        ctx: CallContext,
        method: Ident,
        args: VarArgs,
    ) -> EvalResult<ValueRef> {
        if let Some(method) = self.methods.get(&*method.inner) {
            method.call(args, ctx)
        } else {
            (self.method_fallback)(ctx, method, args)
        }
    }

    pub(crate) fn call_static_method(
        &self,
        ctx: CallContext,
        method: Ident,
        args: VarArgs,
    ) -> EvalResult<ValueRef> {
        if let Some(func) = self.static_methods.get(&*method.inner) {
            func.call(args, ctx)
        } else {
            (self.static_method_fallback)(ctx, method, args)
        }
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
    pub(crate) fn new(engine: Rc<Engine>) -> Self {
        Self {
            inner: AnyRegistry {
                name: T::type_name().as_ref().into(),
                type_id: TypeId::of::<T>(),
                engine,
                methods: Default::default(),
                method_fallback: unknown_method,
                static_methods: Default::default(),
                static_method_fallback: unknown_method,
                fields: Default::default(),
                field_get_fallback: Rc::new(UnknownField),
                field_set_fallback: Rc::new(UnknownField),
                bin_ops: Default::default(),
                unary_ops: Default::default(),
                cmps: Default::default(),
                call: Default::default(),
                indexers: Default::default(),
            },
            _p: PhantomData,
        }
    }

    /// Get a reference to the engine.  This can be used to recursively register types:
    ///
    /// ```ignore
    /// fn register(registry: &mut Registry<MyType>) {
    ///     registry.engine().register_type::<MyOtherType>();
    ///     // ...
    /// }
    /// ```
    pub fn engine(&self) -> Rc<Engine> {
        self.inner.engine.clone()
    }

    pub fn register_method<F>(&mut self, name: &'static str, func: F)
    where
        F: Method<T> + 'static,
    {
        self.inner.methods.insert(name, Rc::new(func));
    }

    pub fn register_method_fallback(
        &mut self,
        func: fn(CallContext, Ident, VarArgs) -> EvalResult<ValueRef>,
    ) {
        self.inner.method_fallback = func;
    }

    pub fn register_static_method<F>(&mut self, name: &'static str, func: F)
    where
        F: Function + 'static,
    {
        self.inner.static_methods.insert(name, Rc::new(func));
    }

    pub fn register_static_method_fallback(
        &mut self,
        func: fn(CallContext, Ident, VarArgs) -> EvalResult<ValueRef>,
    ) {
        self.inner.static_method_fallback = func;
    }

    pub fn register_call<F>(&mut self, func: F)
    where
        F: Method<T> + 'static,
    {
        self.inner.call = Some(Rc::new(func));
    }

    pub fn register_field_get<Ret>(&mut self, name: &'static str, func: fn(CallContext, &T) -> Ret)
    where
        fn(CallContext, &T) -> Ret: Getter + 'static,
    {
        self.inner.fields.insert(
            name,
            Field {
                getter: Rc::new(func),
                setter: None,
            },
        );
    }

    pub fn register_field_get_set<Ret: 'static, SetRet: 'static>(
        &mut self,
        name: &'static str,
        getter: fn(CallContext, &T) -> Ret,
        setter: fn(CallContext, &mut T, ValueRef) -> SetRet,
    ) where
        fn(CallContext, &T) -> Ret: Getter + 'static,
        fn(CallContext, &mut T, ValueRef) -> SetRet: Setter + 'static,
    {
        self.inner.fields.insert(
            name,
            Field {
                getter: Rc::new(getter),
                setter: Some(Rc::new(setter)),
            },
        );
    }

    pub fn register_field_get_fallback<Ret>(&mut self, func: fn(CallContext, &T, Ident) -> Ret)
    where
        fn(CallContext, &T, Ident) -> Ret: FieldGetFallback + 'static,
    {
        self.inner.field_get_fallback = Rc::new(func);
    }

    pub fn register_field_get_set_fallback<Ret: 'static, SetRet: 'static>(
        &mut self,
        getter: fn(CallContext, &T, Ident) -> Ret,
        setter: fn(CallContext, &mut T, Ident, ValueRef) -> SetRet,
    ) where
        fn(CallContext, &T, Ident) -> Ret: FieldGetFallback + 'static,
        fn(CallContext, &mut T, Ident, ValueRef) -> SetRet: FieldSetFallback + 'static,
    {
        self.inner.field_get_fallback = Rc::new(getter);
        self.inner.field_set_fallback = Rc::new(setter);
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
        getter: fn(CallContext<GetIndexCtx>, &T, &IndexType) -> Ret,
        setter: fn(CallContext<SetIndexCtx>, &mut T, &IndexType, ValueRef) -> SR,
    ) where
        fn(CallContext<GetIndexCtx>, &T, &IndexType) -> Ret: IndexGetter + 'static,
        fn(CallContext<SetIndexCtx>, &mut T, &IndexType, ValueRef) -> SR: IndexSetter + 'static,
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

    pub(crate) fn erase(self) -> AnyRegistry {
        self.inner
    }
}
