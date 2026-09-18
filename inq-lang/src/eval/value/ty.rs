use std::{cell::RefCell, rc::Rc};

use crate::eval::{
    registry::{AnyRegistry, Registry},
    value::{Value, ValueRef},
};

#[derive(derive_more::Debug, Clone)]
#[debug("Type({:?}, {:?})", self.reg.name, self.reg.type_id)]
pub struct TypeValue {
    reg: Rc<AnyRegistry>,
}

impl From<Rc<AnyRegistry>> for TypeValue {
    fn from(value: Rc<AnyRegistry>) -> Self {
        Self { reg: value }
    }
}

impl Value for TypeValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Type".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        format!("Type<{}>", self.reg.name).into()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "Type<{}>", self.reg.name).unwrap();
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        // TypeValues are immutable, so we don't need to clone the inner Rc
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "Type<{}>", self.reg.name)
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&mut _, ValueRef) -> _>("is_instance", |this, val| {
            val.type_id() == this.reg.type_id
        });
        registry.register_method_fallback(|ctx, method, args| {
            let this = ctx.self_ref.unwrap::<Self>();
            this.reg.call_static_method(ctx, method, args)
        });
    }
}
