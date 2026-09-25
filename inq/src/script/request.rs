use std::{cell::RefCell, ops::Deref, rc::Rc};

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
    debug_fmt,
    script::{
        bytes_value::BytesValue, client::ClientConfig, header::HeaderMapValue, json::Json,
        url::UrlValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RequestBody {
    None,
    Json(Rc<Json>),
    Text(IStr),
    Bytes(Rc<BytesValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestValue {
    pub method: Method,
    pub url: Rc<UrlValue>,
    pub headers: Rc<HeaderMapValue>,
    pub body: RefCell<RequestBody>,
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
            body: RefCell::new(RequestBody::None),
            client: Rc::new(client.into()),
        }
    }

    pub(crate) fn into_reqwest(self) -> (Request, Client) {
        ((&self).into(), self.client.deref().clone().into())
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
            RequestBody::Bytes(bytes) => {
                *request.body_mut() = Some(Body::from(Vec::from(bytes.deref().clone())));
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
        let body = match &*self.body.borrow() {
            RequestBody::None => ValueRef::null(),
            RequestBody::Json(json) => ValueRef::from_ref(json.clone()),
            RequestBody::Text(istr) => ValueRef::from(istr.clone()),
            RequestBody::Bytes(bytes) => ValueRef::from_ref(bytes.clone()),
        };
        debug_fmt! {
            into f as "Url",
            url     => ValueRef::from_ref(self.url.clone()),
            headers => ValueRef::from_ref(self.headers.clone()),
            client  => ValueRef::from_ref(self.client.clone()),
            body    => body,
        }
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
        registry.register_field_get("url", |_, this| ValueRef::from_ref(this.url.clone()));
        registry.register_field_get("headers", |_, this| {
            ValueRef::from_ref(this.headers.clone())
        });
        registry.register_field_get("client", |_, this| ValueRef::from_ref(this.client.clone()));
        registry.register_field_get_set(
            "body",
            |_, this| match &*this.body.borrow() {
                RequestBody::None => ValueRef::null(),
                RequestBody::Json(json) => ValueRef::from_ref(json.clone()),
                RequestBody::Text(text) => text.clone().into(),
                RequestBody::Bytes(bytes) => ValueRef::from_ref(bytes.clone()),
            },
            |ctx, this, rhs| {
                if rhs.is::<Null>() {
                    *this.body.borrow_mut() = RequestBody::None;
                    this.headers.0.borrow_mut().remove(header::CONTENT_TYPE);
                } else if let Some(json) = rhs.downcast_rc::<Json>() {
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
                } else if let Some(bytes) = rhs.downcast_rc::<BytesValue>() {
                    *this.body.borrow_mut() = RequestBody::Bytes(bytes);
                    this.headers.0.borrow_mut().insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_str("application/octet-stream").unwrap(),
                    );
                } else {
                    return Err(ctx.error("Invalid value for body.  Must be `Json` or `String`"));
                }
                Ok(())
            },
        );
    }
}
