use std::{cell::RefCell, net::SocketAddr, rc::Rc};

use inq_lang::{
    StringExt,
    eval::{
        EvalError,
        registry::Registry,
        value::{CallContext, Value, native::Int},
    },
};
use miette::{Context, IntoDiagnostic};
use reqwest::{StatusCode, Version, blocking::Response, header};

use crate::script::{Encoding, ScriptBody, header::HeaderMapValue, url::UrlValue};

#[derive(Debug, Clone)]
pub struct ResponseValue {
    pub status: u16,
    pub version: Version,
    pub url: UrlValue,
    pub remote_addr: Option<SocketAddr>,
    pub headers: HeaderMapValue,
    pub content_length: Option<u64>,
    pub body: ScriptBody,
}

impl ResponseValue {
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status).expect("set from Response")
    }
}

impl TryFrom<Response> for ResponseValue {
    type Error = miette::Error;
    fn try_from(value: Response) -> miette::Result<Self> {
        Ok(Self {
            status: value.status().as_u16(),
            version: value.version(),
            url: UrlValue(value.url().clone()),
            remote_addr: value.remote_addr(),
            headers: HeaderMapValue(value.headers().clone()),
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

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("status", |_, this| Int::from(this.status));
        registry.register_field_get("version", |_, this| format!("{:?}", this.version).intern());
        registry.register_field_get("url", |_, this| this.url.clone());
        registry.register_field_get("remote_addr", |_, this| {
            this.remote_addr.map(|s| s.to_string().intern())
        });
        registry.register_field_get("headers", |_, this| this.headers.clone());
        registry.register_field_get("content_length", |_, this| {
            this.content_length.map(|n| n as Int).unwrap_or_default()
        });
        registry.register_method::<fn(CallContext, &mut _) -> _>("json", |ctx, this| {
            this.body.json().map_err(|e| EvalError::Custom {
                message: format!("Unable to parse response body as json: {}", e),
                span: ctx.span(),
            })
        });
    }
}
// fn re(mut builder: Registry<Self>) {
//     builder
//         .on_debug(|x| format!("{:#?}", x))
//         .with_get("version", |r: &mut Self| r.version)
//         .with_get("url", |r: &mut Self| r.url.clone())
//         .with_get("remote_addr", |r: &mut Self| r.remote_addr)
//         .with_get("headers", |r: &mut Self| r.headers.clone())
//         .with_get("content_length", |r: &mut Self| r.content_length)
//         .with_fn("json", |r: &mut Self| {
//             r.body.json().map_err(|e| {
//                 Box::new(EvalAltResult::ErrorRuntime(
//                     format!("Unable to parse response body as json: {}", e).into(),
//                     Position::NONE,
//                 ))
//             })
//         })
//         .with_fn("text", |r: &mut Self| {
//             r.body.text().map_err(|e| {
//                 Box::new(EvalAltResult::ErrorRuntime(
//                     format!("Unable to parse response body as text: {}", e).into(),
//                     Position::NONE,
//                 ))
//             })
//         });
// }
