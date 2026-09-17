use std::{cell::RefCell, rc::Rc};

use inq_lang::{
    IStr,
    eval::{
        registry::Registry,
        value::{Value, ValueRef, native::Int},
    },
};
use reqwest::Url;

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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }
    fn truthy(&self) -> bool {
        true
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self.0).unwrap();
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        macro_rules! debug {
            ($e: expr) => {
                std::fmt::from_fn(|fmt| ValueRef::from($e).borrow().debug(fmt))
            };
        }
        fmt.debug_struct("Url")
            .field("host", &debug!(self.0.host_str().map(IStr::from)))
            .field("domain", &debug!(self.0.domain().map(IStr::from)))
            .field("port", &debug!(self.0.port().map(Int::from)))
            .field("path", &debug!(IStr::from(self.0.path())))
            .field("query", &debug!(self.0.query().map(IStr::from)))
            .field("fragment", &debug!(self.0.fragment().map(IStr::from)))
            .field("authority", &debug!(IStr::from(self.0.authority())))
            .field("username", &debug!(IStr::from(self.0.username())))
            .field("password", &debug!(self.0.password().map(IStr::from)))
            .finish()
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("host", |_, this| this.0.host_str().map(IStr::from));
        registry.register_field_get("domain", |_, this| this.0.domain().map(IStr::from));
        registry.register_field_get("port", |_, this| this.0.port().map(Int::from));
        registry.register_field_get::<IStr>("path", |_, this| this.0.path().into());
        registry.register_field_get("query", |_, this| this.0.query().map(IStr::from));
        registry.register_field_get("fragment", |_, this| this.0.fragment().map(IStr::from));
        registry.register_field_get::<IStr>("authority", |_, this| this.0.authority().into());
        registry.register_field_get::<IStr>("username", |_, this| this.0.username().into());
        registry.register_field_get("password", |_, this| this.0.password().map(IStr::from));
    }
}
