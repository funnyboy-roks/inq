use std::{cell::RefCell, rc::Rc, str::FromStr};

use inq_lang::{
    IStr,
    eval::{EvalError, registry::Registry, value::Value},
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

#[derive(Debug, Clone)]
pub(crate) struct HeaderMapValue(pub(crate) HeaderMap);
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
        <HeaderMap as std::fmt::Debug>::fmt(&self.0, fmt)
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
        registry.register_index_get_set(
            |ctx, this, s: &IStr| {
                let name = HeaderName::from_str(s.as_str()).map_err(|_| EvalError::Custom {
                    message: "Invalid header name".into(),
                    span: ctx.index_span,
                })?;
                Ok(this
                    .0
                    .get(name)
                    .map(HeaderValue::to_str)
                    .transpose()
                    .unwrap()
                    .map(IStr::from))
            },
            |ctx, this, s: &IStr, v| {
                let val = v.expect_downcast::<IStr>(ctx.rhs_span)?;
                let name = HeaderName::from_str(s.as_str()).map_err(|_| EvalError::Custom {
                    message: "Invalid header name".into(),
                    span: ctx.index_span,
                })?;
                let value = HeaderValue::from_str(&val).map_err(|_| EvalError::Custom {
                    message: "Invalid header value".into(),
                    span: ctx.rhs_span,
                })?;
                this.0.insert(name, value);
                Ok(())
            },
        );
    }
}
