use std::rc::Rc;

use indexmap::IndexMap;
use inq_lang::{
    IStr,
    eval::{
        EvalResult,
        registry::{FnCtx, Registry},
        value::{CallContext, Value, ValueRef, native::Object},
    },
};
use ureq::unversioned::multipart::{Form as UreqForm, Part};

use crate::script::bytes_value::BytesValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Field {
    Text(IStr),
    Bytes(Vec<u8>),
}

type FormInner = IndexMap<IStr, Field>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Form(pub(crate) FormInner);
impl Value for Form {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Form".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{:?}", self.0).unwrap();
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "Form({:?})", self.0)
    }

    fn truthy(&self) -> bool {
        true
    }
    fn eq(&self, other: ValueRef) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }

    fn register(_: &mut Registry<Self>)
    where
        Self: Sized,
    {
    }
}

impl Form {
    pub(crate) fn to_ureq(&self) -> UreqForm<'_> {
        let mut form = UreqForm::new();
        for (key, value) in self.0.iter() {
            form = match value {
                Field::Text(text) => form.text(key.as_str(), text.as_str()),
                Field::Bytes(bytes) => form.part(key.as_str(), Part::bytes(bytes)),
            };
        }
        form
    }

    pub fn from_value(ctx: CallContext<FnCtx>, arg: Object) -> EvalResult<Self> {
        let mut this = FormInner::new();

        for (key, value) in arg.0.borrow().iter() {
            if let Some(text) = value.downcast_ref::<IStr>() {
                this.insert(key.clone(), Field::Text(text.clone()));
            } else if let Some(bytes) = value.downcast_ref::<BytesValue>() {
                this.insert(key.clone(), Field::Bytes(bytes.clone().into()));
            } else {
                return Err(ctx.error_arg(
                    0,
                    format!(
                        "Expected object value to be String, got {} at key '{}'",
                        value.type_name_of(),
                        key
                    ),
                ));
            }
        }

        Ok(Self(this))
    }
}
