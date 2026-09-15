use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::{OnceCell, Ref, RefCell, RefMut},
    fmt::{Debug, Display},
    ops::Deref,
    rc::Rc,
};

use crate::{
    Span,
    eval::{EvalError, EvalResult, registry::Registry, value::native::Null},
};

/// Native types
pub mod native;

#[derive(Clone)]
pub struct ValueRef(Rc<RefCell<dyn Value>>);

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
        write!(f, "{:?}", self.borrow())
    }
}

impl ValueRef {
    pub fn new(value: impl Value) -> Self {
        Self(Rc::new(RefCell::new(value)))
    }

    pub fn from_ref(value: Rc<RefCell<impl Value>>) -> Self {
        Self(value)
    }

    pub fn null() -> Self {
        thread_local! {
            static NULL: OnceCell<ValueRef> = const { OnceCell::new() };
        }
        NULL.with(|null| null.get_or_init(|| Self::new(Null)).clone())
    }

    pub fn type_id(&self) -> TypeId {
        let b = self.0.borrow();
        (&*b as &dyn Any).type_id()
    }

    pub fn borrow(&self) -> Ref<'_, dyn Value> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, dyn Value> {
        self.0.borrow_mut()
    }

    pub fn type_name_of(&self) -> Cow<'static, str> {
        self.borrow().type_name_of()
    }

    pub fn downcast<T: Value + Clone>(&self) -> Option<T> {
        self.borrow().downcast_ref().cloned()
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
                self.borrow().type_name_of()
            );
        }
    }

    pub fn is<T: Value>(&self) -> bool {
        self.borrow().is::<T>()
    }

    pub fn expect_downcast<T: Value + Clone>(&self, span: Span) -> EvalResult<T> {
        self.downcast::<T>().ok_or_else(|| EvalError::InvalidType {
            expected: T::type_name().into(),
            actual: self.borrow().type_name_of().into(),
            span,
        })
    }

    pub fn debug(&self) -> impl Debug + use<'_> {
        struct VDebug<'a>(&'a ValueRef);
        impl Debug for VDebug<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.borrow().debug(f)
            }
        }
        VDebug(self)
    }

    pub fn display(&self) -> impl Display + use<'_> {
        struct VDisplay<'a>(&'a ValueRef);
        impl Display for VDisplay<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut s = String::new();
                self.0.borrow().to_string(&mut s);
                write!(f, "{}", s)
            }
        }
        VDisplay(self)
    }

    pub(crate) fn snapshot(&self) -> Self {
        Self(self.borrow().snapshot())
    }
}

#[derive(Clone, Debug)]
pub struct CallContext<T = ()> {
    /// The span that makes the most sense for a single error label
    pub(crate) span: Span,
    pub(crate) self_ref: ValueRef,
    ext: T,
}

impl CallContext<()> {
    pub fn new(span: Span, self_ref: ValueRef) -> Self {
        Self {
            span,
            self_ref,
            ext: (),
        }
    }
}

impl<T> CallContext<T> {
    pub fn span(&self) -> Span {
        self.span
    }

    pub fn new_ext(span: Span, self_ref: ValueRef, ext: T) -> Self {
        Self {
            span,
            self_ref,
            ext,
        }
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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>>;
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
    fn truthy(&self) -> bool;

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
