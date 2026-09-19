use std::{cell::RefCell, rc::Rc};

use inq_lang::{
    IStr,
    eval::{
        registry::Registry,
        value::{Value, ValueRef, native::Null},
    },
};
use reqwest::{
    Method, Url,
    blocking::{Body, Client, Request},
    header::{self, HeaderMap, HeaderName, HeaderValue},
};

use crate::{
    cli::CliClientConfig,
    script::{client::ClientConfig, header::HeaderMapValue, json::Json, url::UrlValue},
};

#[derive(Debug, Clone)]
pub(crate) enum RequestBody {
    None,
    Json(Json),
    Text(IStr),
}

#[derive(Debug, Clone)]
pub(crate) struct RequestValue {
    pub method: Method,
    pub url: Rc<UrlValue>,
    pub headers: Rc<HeaderMapValue>,
    pub body: Rc<RefCell<RequestBody>>,
    pub client: Rc<ClientConfig>,
}

impl RequestValue {
    pub(crate) fn new(client: CliClientConfig, method: Method, url: Url) -> Self {
        Self {
            method,
            url: Rc::new(UrlValue(url)),
            headers: Rc::new(HeaderMapValue(RefCell::new(HeaderMap::from_iter([(
                HeaderName::from_static("user-agent"),
                HeaderValue::from_static(concat!("inq/", env!("CARGO_PKG_VERSION"))),
            )])))),
            body: Rc::new(RefCell::new(RequestBody::None)),
            client: Rc::new(client.into()),
        }
    }

    pub(crate) fn into_reqwest(self) -> (Request, Client) {
        ((&self).into(), (&*self.client).clone().into())
    }
}

impl From<&RequestValue> for Request {
    fn from(value: &RequestValue) -> Self {
        let mut request = Request::new(value.method.clone(), value.url.0.clone());
        *request.headers_mut() = value.headers.0.borrow().clone();
        match &*value.body.borrow() {
            RequestBody::None => {}
            RequestBody::Json(json) => {
                *request.body_mut() = Some(Body::from(json.0.borrow().to_string()));
            }
            RequestBody::Text(istr) => {
                *request.body_mut() = Some(Body::from(istr.as_str().to_string()));
            }
        }
        request
    }
}

impl Value for RequestValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Request".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{} {}", self.method, self.url.0).unwrap();
    }
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, f)
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("url", |_, this| ValueRef::from_ref(this.url.clone()));
        registry.register_field_get("headers", |_, this| {
            ValueRef::from_ref(this.headers.clone())
        });
        registry.register_field_get("client", |_, this| ValueRef::from_ref(this.client.clone()));
        registry.register_field_get_set(
            "body",
            |_, this| match &*this.body.borrow() {
                RequestBody::None => ValueRef::null(),
                RequestBody::Json(json) => json.clone().into(),
                RequestBody::Text(text) => text.clone().into(),
            },
            |ctx, this, rhs| {
                if rhs.is::<Null>() {
                    *this.body.borrow_mut() = RequestBody::None;
                    this.headers.0.borrow_mut().remove(header::CONTENT_TYPE);
                } else if let Some(json) = rhs.downcast::<Json>() {
                    *this.body.borrow_mut() = RequestBody::Json(json);
                    this.headers.0.borrow_mut().insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    );
                } else if let Some(text) = rhs.downcast::<IStr>() {
                    *this.body.borrow_mut() = RequestBody::Text(text);
                    this.headers.0.borrow_mut().insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_str("text/plain; charset=utf-8").unwrap(),
                    );
                } else {
                    return Err(ctx.error("Invalid value for body.  Must be `Json` or `String`"));
                }
                Ok(())
            },
        );
    }
}
