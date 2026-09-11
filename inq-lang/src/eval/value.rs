use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::{OnceCell, RefCell},
    fmt::Debug,
    ops::Deref,
    rc::Rc,
};

use crate::{
    Span,
    eval::{registry::Registry, value::native::Null},
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

impl Debug for ValueRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.borrow())
    }
}

impl ValueRef {
    pub fn new(value: impl Value) -> Self {
        Self(Rc::new(RefCell::new(value)))
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

    pub fn type_name_of(&self) -> Cow<'static, str> {
        self.borrow().type_name_of()
    }

    pub fn downcast<T: Value + Clone>(&self) -> Option<T> {
        self.borrow().downcast_ref().cloned()
    }

    pub fn is<T: Value>(&self) -> bool {
        self.borrow().is::<T>()
    }
}

impl Deref for ValueRef {
    type Target = Rc<RefCell<dyn Value>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct CallContext {
    pub(crate) span: Span,
    pub(crate) self_ref: ValueRef,
}

impl CallContext {
    pub fn span(&self) -> Span {
        self.span
    }
}

pub trait Value: Any + Debug {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized;
    fn type_name_of(&self) -> Cow<'static, str>;
    fn to_string(&self, out: &mut String);
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
}
