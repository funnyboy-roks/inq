use std::{cell::RefCell, rc::Rc};

use inq_lang::{
    IStr,
    eval::{
        EvalError, EvalResult,
        registry::Registry,
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Float, Int, Null, Object},
        },
    },
};

#[derive(Debug, Clone)]
pub(crate) struct Json(pub serde_json::Value);
impl Value for Json {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Json".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self.0).unwrap();
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }

    fn truthy(&self) -> bool {
        true
    }

    fn register(_registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
    }
}

impl Json {
    pub fn from_value(ctx: CallContext, arg: ValueRef) -> EvalResult<Self> {
        Json::json_inner(&ctx, arg).map(Self)
    }

    pub fn json_inner(ctx: &CallContext, arg: ValueRef) -> EvalResult<serde_json::Value> {
        #[allow(clippy::redundant_pattern_matching)]
        if let Some(s) = arg.borrow().downcast_ref::<IStr>() {
            Ok(serde_json::Value::String(s.into()))
        } else if let Some(i) = arg.downcast::<Int>() {
            Ok(serde_json::Value::from(i))
        } else if let Some(f) = arg.downcast::<Float>() {
            Ok(serde_json::Value::from(f))
        } else if let Some(b) = arg.downcast::<bool>() {
            Ok(serde_json::Value::from(b))
        } else if let Some(_) = arg.downcast::<Null>() {
            Ok(serde_json::Value::from(()))
        } else if let Some(a) = arg.downcast::<Array>() {
            a.into_iter().map(|a| Self::json_inner(ctx, a)).collect()
        } else if let Some(o) = arg.downcast::<Object>() {
            o.0.into_iter()
                .map(|(k, v)| {
                    let x = Self::json_inner(ctx, v)?;
                    Ok((k.as_str().to_string(), x))
                })
                .collect()
        } else {
            Err(EvalError::Custom {
                message: format!("Invalid type for JSON: {}", arg.type_name_of()),
                span: ctx.span(),
            })
        }
    }
}
