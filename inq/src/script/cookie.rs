use std::{cell::RefCell, rc::Rc, str::FromStr};

use cookie::Cookie;
use inq_lang::{
    IStr, StringExt,
    eval::value::{CallContext, Value},
};

use crate::script::datetime::DateTimeValue;

#[derive(Debug, Clone)]
pub struct CookieValue {
    inner: Cookie<'static>,
}

impl Value for CookieValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Cookie".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str("Cookie")
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Cookie as std::fmt::Debug>::fmt(&self.inner, fmt)
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_static_method::<fn(_, _) -> _>("parse", |ctx: CallContext, s: IStr| {
            Ok(Self {
                inner: Cookie::from_str(&s)
                    .map_err(|e| ctx.error(format!("Unable to parse cookie: {}", e)))?,
            })
        });
        registry.register_field_get("name", |_, this| this.inner.name().intern());
        registry.register_field_get("value", |_, this| this.inner.value().intern());
        registry.register_field_get("expires_at", |_, this| {
            this.inner.expires_datetime().map(DateTimeValue::from)
        });
    }
}
