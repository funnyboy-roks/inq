use std::{cell::RefCell, ops::Deref, rc::Rc};

use inq_lang::{
    IStr,
    eval::{
        registry::Registry,
        value::{Value, ValueRef, native::Null},
    },
};
use ureq::{
    Agent,
    http::{self, HeaderMap, HeaderName, HeaderValue, Request, header},
};
use url::Url;

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
    pub method: http::Method,
    pub url: Rc<UrlValue>,
    pub headers: Rc<HeaderMapValue>,
    pub body: RefCell<RequestBody>,
    pub client: Rc<ClientConfig>,
}

impl RequestValue {
    pub(crate) fn new(client: CliClientConfig, method: http::Method, url: Url) -> Self {
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

    pub(crate) fn into_ureq(self) -> (Request<Vec<u8>>, Agent) {
        ((&self).into(), self.client.deref().clone().into())
    }
}

impl From<&RequestValue> for Request<Vec<u8>> {
    fn from(value: &RequestValue) -> Self {
        let mut request = Request::builder()
            .method(value.method.clone())
            .uri(value.url.0.as_str());
        *request.headers_mut().unwrap() = value.headers.0.borrow().clone();
        let request = match &*value.body.borrow() {
            RequestBody::None => request.body(Vec::new()),
            RequestBody::Json(json) => request.body(json.0.borrow().to_string().into_bytes()),
            RequestBody::Text(istr) => request.body(istr.as_str().as_bytes().to_vec()),
            RequestBody::Bytes(bytes) => request.body(Vec::from(bytes.deref().clone())),
        };
        request.expect("Invalid request") // TODO: handle this error
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
