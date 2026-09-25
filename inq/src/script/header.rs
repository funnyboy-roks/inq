use std::{cell::RefCell, rc::Rc, str::FromStr};

use inq_lang::{
    IStr,
    eval::{
        EvalError,
        registry::Registry,
        value::{Value, ValueRef, native::Int},
    },
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeaderMapValue(pub(crate) RefCell<HeaderMap>);
impl Value for HeaderMapValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "HeaderMap".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{:?}", self).unwrap();
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <HeaderMap as std::fmt::Debug>::fmt(&self.0.borrow(), fmt)
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn eq(&self, other: ValueRef) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&_) -> _>("len", |this| this.0.borrow().len() as Int);
        registry.register_index_get_set(
            |ctx, this, s: &IStr| {
                let name = HeaderName::from_str(s.as_str())
                    .map_err(|_| EvalError::custom(ctx.index_span, "Invalid header name"))?;
                Ok(this
                    .0
                    .borrow()
                    .get(name)
                    .map(HeaderValue::to_str)
                    .transpose()
                    .unwrap()
                    .map(IStr::from))
            },
            |ctx, this, s: &IStr, v| {
                let val = v.expect_downcast::<IStr>(ctx.rhs_span)?;
                let name = HeaderName::from_str(s.as_str())
                    .map_err(|_| EvalError::custom(ctx.index_span, "Invalid header name"))?;
                let value = HeaderValue::from_str(&val)
                    .map_err(|_| EvalError::custom(ctx.rhs_span, "Invalid header value"))?;
                this.0.borrow_mut().insert(name, value);
                Ok(())
            },
        );
    }
}
