use std::{cell::RefCell, net::SocketAddr, rc::Rc};

use inq_lang::{
    IStr, StringExt,
    eval::{
        registry::{FnCtx, Registry},
        value::{CallContext, Value, ValueRef, native::Int},
    },
};
use miette::{Context, IntoDiagnostic};
use reqwest::{StatusCode, Version, blocking::Response, header};

use crate::{
    debug_fmt,
    script::{
        Encoding, ScriptBody, bytes_value::BytesValue, header::HeaderMapValue, url::UrlValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseValue {
    pub status: Int,
    pub version: Version,
    pub url: UrlValue,
    pub remote_addr: Option<SocketAddr>,
    pub headers: HeaderMapValue,
    pub content_length: Option<u64>,
    pub body: ScriptBody,
}

impl ResponseValue {
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status as _).expect("set from Response")
    }
}

impl TryFrom<Response> for ResponseValue {
    type Error = miette::Error;
    fn try_from(value: Response) -> miette::Result<Self> {
        Ok(Self {
            status: value.status().as_u16().into(),
            version: value.version(),
            url: UrlValue(value.url().clone()),
            remote_addr: value.remote_addr(),
            headers: HeaderMapValue(RefCell::new(value.headers().clone())),
            content_length: value.content_length(),
            body: {
                let encoding = value.headers().get(header::CONTENT_ENCODING);

                let encoding = if let Some(encoding) = encoding {
                    let encoding = encoding
                        .to_str()
                        .into_diagnostic()
                        .context("Unable to decode Content-Encoding header")?;
                    Some(Encoding::from_str(encoding))
                } else {
                    None
                };

                let content_type = value.headers().get(header::CONTENT_TYPE);

                let content_type = if let Some(content_type) = content_type {
                    let content_type = content_type
                        .to_str()
                        .into_diagnostic()
                        .context("Unable to decode Content-Type header")?;
                    Some(content_type.into())
                } else {
                    None
                };

                ScriptBody {
                    encoding,
                    content_type,
                    bytes: value.bytes().into_diagnostic()?,
                }

                // if !raw
                //     && let Some(header) = res.headers().get(header::CONTENT_TYPE)
                //     && header == "application/json"
                // {
                //     ScriptBody::Json(res.json().into_diagnostic()?)
                // } else {
                //     ScriptBody::Text(res.text().into_diagnostic()?)
                // }
            },
        })
    }
}

impl Value for ResponseValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Response".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "<response>").unwrap();
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn eq(&self, other: ValueRef) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_fmt! {
            into fmt as "Response",
            status         => self.status,
            version        => format!("{:?}", self.version).intern(),
            url            => self.url.clone(),
            remote_addr    => self.remote_addr.map(|s| s.to_string().intern()),
            headers        => self.headers.clone(),
            content_length => self.content_length.map(|n| n as Int).unwrap_or_default(),
        }
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("status", |_, this| this.status);
        registry.register_field_get("version", |_, this| format!("{:?}", this.version).intern());
        registry.register_field_get("url", |_, this| this.url.clone());
        registry.register_field_get("remote_addr", |_, this| {
            this.remote_addr.map(|s| s.to_string().intern())
        });
        registry.register_field_get("headers", |_, this| this.headers.clone());
        registry.register_field_get("content_length", |_, this| {
            this.content_length.map(|n| n as Int).unwrap_or_default()
        });
        registry.register_method::<fn(CallContext<FnCtx>, &_) -> _>("json", |ctx, this| {
            this.body
                .json()
                .map_err(|e| ctx.error(format!("Unable to parse response body as json: {}", e)))
        });
        registry.register_method::<fn(CallContext<FnCtx>, &_) -> _>("text", |ctx, this| {
            this.body
                .text()
                .map_err(|e| ctx.error(format!("Unable to parse response body as text: {}", e)))
                .map(IStr::from)
        });
        registry.register_method::<fn(CallContext<FnCtx>, &_) -> _>("bytes", |ctx, this| {
            this.body
                .bytes()
                .map_err(|e| ctx.error(format!("{}", e)))
                .map(BytesValue::from)
        });
    }
}
