use std::rc::Rc;

use inq_lang::{
    IStr, StringExt,
    eval::{
        registry::Registry,
        value::{Value, native::Int},
    },
};
use reqwest::Url;

use crate::debug_fmt;

#[derive(Debug, Clone)]
pub struct UrlValue(pub(crate) Url);
impl Value for UrlValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Url".into()
    }
    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn truthy(&self) -> bool {
        true
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self.0).unwrap();
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_fmt! {
            into fmt as "Url",
            host      => self.0.host_str().map(IStr::from),
            domain    => self.0.domain().map(IStr::from),
            port      => self.0.port().map(Int::from),
            path      => self.0.path().intern(),
            query     => self.0.query().map(IStr::from),
            fragment  => self.0.fragment().map(IStr::from),
            authority => self.0.authority().intern(),
            username  => self.0.username().intern(),
            password  => self.0.password().map(IStr::from),
        }
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("host", |_, this| this.0.host_str().map(IStr::from));
        registry.register_field_get("domain", |_, this| this.0.domain().map(IStr::from));
        registry.register_field_get("scheme", |_, this| this.0.scheme().intern());
        registry.register_field_get("port", |_, this| this.0.port().map(Int::from));
        registry.register_field_get("path", |_, this| this.0.path().intern());
        registry.register_field_get("query", |_, this| this.0.query().map(IStr::from));
        registry.register_field_get("fragment", |_, this| this.0.fragment().map(IStr::from));
        registry.register_field_get("authority", |_, this| this.0.authority().intern());
        registry.register_field_get("username", |_, this| this.0.username().intern());
        registry.register_field_get("password", |_, this| this.0.password().map(IStr::from));
    }
}
