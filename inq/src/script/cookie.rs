use std::{rc::Rc, str::FromStr};

use cookie::Cookie;
use inq_lang::{IStr, StringExt, eval::value::Value};

use crate::{debug_fmt, script::datetime::DateTimeValue};

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

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_fmt! {
            into fmt as "Cookie",
            name       => self.inner.name().intern(),
            value      => self.inner.value().intern(),
            expires_at => self.inner.expires_datetime().map(DateTimeValue::from),
        }
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_static_method("parse", |ctx, s: IStr| {
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
