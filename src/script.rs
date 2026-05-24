use std::net::SocketAddr;

use miette::IntoDiagnostic;
use reqwest::{
    StatusCode, Version,
    blocking::Response,
    header::{self, HeaderMap, HeaderValue},
};
use rhai::{CustomType, Dynamic, Engine, EvalAltResult, Position, TypeBuilder};

#[derive(Debug, Clone)]
pub enum ScriptBody {
    Text(String),
    Json(serde_json::Value),
}

#[derive(Debug, Clone)]
pub struct ScriptResponse {
    pub status: u16,
    pub version: Version,
    pub url: String,
    pub remote_addr: Option<SocketAddr>,
    pub headers: HeaderMap<HeaderValue>,
    pub content_length: Option<u64>,
    pub body: ScriptBody,
}

impl CustomType for ScriptResponse {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .on_debug(|x| format!("{:#?}", x))
            .with_get("status", |r: &mut Self| r.status)
            .with_get("version", |r: &mut Self| r.version)
            .with_get("url", |r: &mut Self| r.url.clone())
            .with_get("remote_addr", |r: &mut Self| r.remote_addr)
            .with_get("headers", |r: &mut Self| r.headers.clone())
            .with_get("content_length", |r: &mut Self| r.content_length)
            .with_fn("json", |r: &mut Self| match &r.body {
                ScriptBody::Text(_) => Err(Box::new(EvalAltResult::ErrorRuntime(
                    Dynamic::from("Expected Json, found Text"),
                    Position::NONE,
                ))),
                ScriptBody::Json(v) => rhai::serde::to_dynamic(v.clone()),
            })
            .with_fn("text", |r: &mut Self| match &r.body {
                ScriptBody::Text(t) => Ok(t.clone()),
                ScriptBody::Json(_) => Err(Box::new(EvalAltResult::ErrorRuntime(
                    Dynamic::from("Expected Text, found Json"),
                    Position::NONE,
                ))),
            });
    }
}

impl ScriptResponse {
    pub fn from_response(res: Response, raw: bool) -> miette::Result<Self> {
        Ok(Self {
            status: res.status().as_u16(),
            version: res.version(),
            url: res.url().as_str().into(),
            remote_addr: res.remote_addr(),
            headers: res.headers().clone(),
            content_length: res.content_length(),
            body: if !raw
                && let Some(header) = res.headers().get(header::CONTENT_TYPE)
                && header == "application/json"
            {
                ScriptBody::Json(res.json().into_diagnostic()?)
            } else {
                ScriptBody::Text(res.text().into_diagnostic()?)
            },
        })
    }

    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status).expect("set from Response")
    }
}
