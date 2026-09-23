use std::{
    any::{Any, TypeId},
    borrow::Cow,
    fmt::{Debug, Display},
    ops::Deref,
    rc::Rc,
};

use crate::{
    IStr, Span,
    eval::{
        EvalError, EvalResult, Scope,
        registry::{FnCtx, Registry},
        value::{
            native::{Float, Int, Null},
            ty::TypeValue,
        },
    },
};

/// Native types
pub mod native;
pub mod ty;

#[derive(Clone)]
enum Primitive {
    Int(Int),
    Float(Float),
    Bool(bool),
    Null(Null),
    IStr(IStr),
    Type(TypeValue),
}

impl Primitive {
    fn new(v: &dyn Value) -> Option<Self> {
        match () {
            () if let Some(r) = v.downcast_ref::<Int>() => Some(Self::Int(*r)),
            () if let Some(r) = v.downcast_ref::<Float>() => Some(Self::Float(*r)),
            () if let Some(r) = v.downcast_ref::<bool>() => Some(Self::Bool(*r)),
            () if let Some(r) = v.downcast_ref::<Null>() => Some(Self::Null(r.clone())),
            () if let Some(r) = v.downcast_ref::<IStr>() => Some(Self::IStr(r.clone())),
            () if let Some(r) = v.downcast_ref::<TypeValue>() => Some(Self::Type(r.clone())),
            _ => None,
        }
    }

    fn value(&self) -> &dyn Value {
        match self {
            Primitive::Int(r) => r,
            Primitive::Float(r) => r,
            Primitive::Bool(r) => r,
            Primitive::Null(r) => r,
            Primitive::IStr(r) => r,
            Primitive::Type(r) => r,
        }
    }

    fn is<T: Value>() -> bool {
        TypeId::of::<T>() == TypeId::of::<Int>()
            || TypeId::of::<T>() == TypeId::of::<Float>()
            || TypeId::of::<T>() == TypeId::of::<bool>()
            || TypeId::of::<T>() == TypeId::of::<Null>()
            || TypeId::of::<T>() == TypeId::of::<IStr>()
            || TypeId::of::<T>() == TypeId::of::<TypeValue>()
    }
}

#[derive(Clone)]
enum ValueRefInner {
    Primitive(Primitive),
    Dyn(Rc<dyn Value>),
}

#[derive(Clone)]
pub struct ValueRef {
    inner: ValueRefInner,
}

impl<V: Value> From<V> for ValueRef {
    fn from(value: V) -> Self {
        Self::new(value)
    }
}

impl<V: Into<ValueRef>> From<Option<V>> for ValueRef {
    fn from(value: Option<V>) -> Self {
        value.map(Into::into).unwrap_or_else(ValueRef::null)
    }
}

impl Debug for ValueRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.value())
    }
}

impl ValueRef {
    pub fn new(value: impl Value) -> Self {
        let inner = if let Some(p) = Primitive::new(&value) {
            ValueRefInner::Primitive(p)
        } else {
            ValueRefInner::Dyn(Rc::new(value))
        };
        Self { inner }
    }

    pub fn from_ref(value: Rc<impl Value>) -> Self {
        let inner = if let Some(p) = Primitive::new(&*value) {
            ValueRefInner::Primitive(p)
        } else {
            ValueRefInner::Dyn(value)
        };
        Self { inner }
    }

    pub fn null() -> Self {
        Self {
            inner: ValueRefInner::Primitive(Primitive::Null(Null)),
        }
    }

    pub fn value(&self) -> &dyn Value {
        match self.inner {
            ValueRefInner::Primitive(ref p) => p.value(),
            ValueRefInner::Dyn(ref v) => &**v,
        }
    }

    pub fn type_id(&self) -> TypeId {
        let b = self.value();
        (b as &dyn Any).type_id()
    }

    pub fn type_name_of(&self) -> Cow<'static, str> {
        self.value().type_name_of()
    }

    pub fn downcast<T: Value + Clone>(&self) -> Option<T> {
        self.value().downcast_ref().cloned()
    }

    pub fn downcast_rc<T: Value>(&self) -> Option<Rc<T>> {
        if Primitive::is::<T>() {
            panic!("Cannot call downcast_rc on Primitive type");
        }
        match self.inner {
            ValueRefInner::Primitive(_) => None,
            ValueRefInner::Dyn(ref value) => Rc::downcast::<T>(value.clone()).ok(),
        }
    }

    pub fn downcast_ref<T: Value + Clone>(&self) -> Option<&T> {
        self.value().downcast_ref()
    }

    /// Downcast or panic if self is not T
    #[track_caller]
    pub fn unwrap<T: Value + Clone>(&self) -> T {
        if let Some(t) = self.downcast() {
            t
        } else {
            panic!(
                "Expected type {} got type {}",
                T::type_name(),
                self.type_name_of()
            );
        }
    }

    pub fn is<T: Value>(&self) -> bool {
        self.value().is::<T>()
    }

    pub fn expect_downcast<T: Value + Clone>(&self, span: Span) -> EvalResult<T> {
        self.downcast::<T>().ok_or_else(|| EvalError::InvalidType {
            expected: T::type_name().into(),
            actual: self.value().type_name_of().into(),
            span,
        })
    }

    pub fn debug(&self) -> impl Debug + use<'_> {
        struct VDebug<'a>(&'a ValueRef);
        impl Debug for VDebug<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.value().debug(f)
            }
        }
        VDebug(self)
    }

    pub fn display(&self) -> impl Display + use<'_> {
        struct VDisplay<'a>(&'a ValueRef);
        impl Display for VDisplay<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut s = String::new();
                self.0.value().to_string(&mut s);
                write!(f, "{}", s)
            }
        }
        VDisplay(self)
    }

    pub(crate) fn snapshot(&self) -> Self {
        let inner = match &self.inner {
            ValueRefInner::Primitive(p) => ValueRefInner::Primitive(p.clone()),
            ValueRefInner::Dyn(d) => ValueRefInner::Dyn(d.snapshot()),
        };
        Self { inner }
    }
}

#[derive(Clone, Debug)]
pub struct CallContext<T = ()> {
    /// The span that makes the most sense for a single error label
    pub(crate) span: Span,
    pub(crate) self_ref: ValueRef,
    pub(crate) scope: Rc<Scope>,
    ext: T,
}

impl CallContext<()> {
    pub fn new(span: Span, self_ref: ValueRef, scope: Rc<Scope>) -> Self {
        Self {
            span,
            self_ref,
            scope,
            ext: (),
        }
    }
}

impl<T> CallContext<T> {
    pub fn span(&self) -> Span {
        self.span
    }

    pub fn scope(&self) -> Rc<Scope> {
        self.scope.clone()
    }

    pub fn new_ext(span: Span, self_ref: ValueRef, scope: Rc<Scope>, ext: T) -> Self {
        Self {
            span,
            self_ref,
            scope,
            ext,
        }
    }

    /// Create an error at the location of [`Self::span`]
    pub fn error(&self, message: impl Into<String>) -> EvalError {
        EvalError::Custom {
            message: message.into(),
            span: self.span(),
        }
    }
}

impl CallContext<FnCtx> {
    /// Create an error pointing to a specific argument of this function call.
    ///
    /// Panics if arg is out of bounds for `arg_spans`
    pub fn error_arg(&self, arg: usize, message: impl Display) -> EvalError {
        EvalError::Custom {
            message: message.to_string(),
            span: self.arg_spans[arg],
        }
    }
    /// Create an error pointing to a specific argument of this function call.
    ///
    /// Panics if arg is out of bounds for `arg_spans`
    pub fn error_args(
        &self,
        message: impl Display,
        labels: impl IntoIterator<Item = (usize, impl Into<String>)>,
    ) -> EvalError {
        EvalError::Transparent(miette::miette! {
            labels = labels.into_iter().map(|(n, msg)| self.arg_spans[n].with_label(msg)).collect::<Vec<_>>(),
            "{}", message
        })
    }
}

impl<T> Deref for CallContext<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}

pub trait Value: Any + Debug {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized;
    fn type_name_of(&self) -> Cow<'static, str>;
    fn to_string(&self, out: &mut String);
    /// Take a snapshot of the value in its current state.  The value returned should not change
    /// once returned.
    fn snapshot(&self) -> Rc<dyn Value>;
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized;
}

impl dyn Value {
    pub fn is<T: Any>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }

    pub fn downcast_ref<T: Value>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }

    pub fn downcast_mut<T: Value>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }

    #[track_caller]
    pub fn unwrap_ref<T: Value>(&self) -> &T {
        if let Some(t) = self.downcast_ref() {
            t
        } else {
            panic!(
                "Expected type {} got type {}",
                T::type_name(),
                self.type_name_of()
            );
        }
    }

    #[track_caller]
    pub fn unwrap_mut<T: Value>(&mut self) -> &mut T {
        let name = self.type_name_of();
        if let Some(t) = self.downcast_mut() {
            t
        } else {
            panic!("Expected type {} got type {}", T::type_name(), name,);
        }
    }
}
