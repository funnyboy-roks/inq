use std::{
    cell::RefCell, collections::HashMap, fmt::Display, net::SocketAddr, ops::Deref, rc::Rc,
    str::FromStr,
};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use cookie::Cookie;
use miette::{Context, IntoDiagnostic, bail};
use reqwest::{
    StatusCode, Version,
    blocking::Response,
    header::{CONTENT_ENCODING, CONTENT_TYPE, HeaderMap},
};
use rhai::{
    CustomType, Dynamic, Engine, EvalAltResult, ImmutableString, Position, Scope, TypeBuilder,
    packages::Package,
};
use rhai_fs::FilesystemPackage;
use rhai_rand::RandomPackage;
use serde_json::Value as JsonValue;

use crate::{
    config::Config,
    decode::{decode_brotli, decode_deflate, decode_gzip, decode_zstd},
    state::{PersistedVariable, State},
    util::Interpolated,
};

#[derive(Debug, Clone)]
pub struct Headers {
    headers: HeaderMap,
}

impl Deref for Headers {
    type Target = HeaderMap;

    fn deref(&self) -> &Self::Target {
        &self.headers
    }
}

impl CustomType for Headers {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Headers")
            .with_fn("contains", |this: &mut Self, key: &str| {
                this.headers.contains_key(key)
            })
            .with_indexer_get(|this: &mut Self, key: &str| {
                if let Some(header) = this.headers.get(key) {
                    match header.to_str() {
                        Ok(h) => Ok(h.to_string()),
                        Err(e) => Err(Box::new(EvalAltResult::ErrorSystem(
                            "Unable to parse header value".to_string(),
                            Box::new(e),
                        ))),
                    }
                } else {
                    Err(Box::new(EvalAltResult::ErrorIndexNotFound(
                        key.into(),
                        Position::NONE,
                    )))
                }
            });
    }
}

#[derive(Debug, Clone)]
pub enum Encoding {
    Gzip,
    Brotli,
    Zstd,
    Deflate,
    Unknown(String),
}

impl Encoding {
    fn decode(&self, buf: &[u8]) -> miette::Result<Vec<u8>> {
        let mut out = Vec::new();
        match self {
            Encoding::Gzip => decode_gzip(buf, &mut out)?,
            Encoding::Brotli => decode_brotli(buf, &mut out)?,
            Encoding::Zstd => decode_zstd(buf, &mut out)?,
            Encoding::Deflate => decode_deflate(buf, &mut out)?,
            Encoding::Unknown(e) => bail!("Unknown encoding: {}", e),
        }
        Ok(out)
    }

    fn from_str(s: &str) -> Self {
        match () {
            () if s.eq_ignore_ascii_case("gzip") => Self::Gzip,
            () if s.eq_ignore_ascii_case("br") => Self::Brotli,
            () if s.eq_ignore_ascii_case("zstd") => Self::Zstd,
            () if s.eq_ignore_ascii_case("deflate") => Self::Deflate,
            () => Self::Unknown(s.into()),
        }
    }
}

impl Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gzip => write!(f, "gzip"),
            Self::Brotli => write!(f, "br"),
            Self::Zstd => write!(f, "zstd"),
            Self::Deflate => write!(f, "deflate"),
            Self::Unknown(e) => write!(f, "Unknown ({})", e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Json,
    Text,
    Unknown(String),
}

impl ContentType {
    fn from_str(s: &str) -> Self {
        match () {
            () if s.eq_ignore_ascii_case("application/json") => Self::Json,
            () if s.eq_ignore_ascii_case("text/plain") => Self::Text,
            () => Self::Unknown(s.into()),
        }
    }
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "Json"),
            Self::Text => write!(f, "Text"),
            Self::Unknown(e) => write!(f, "Unknown ({})", e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScriptBody {
    encoding: Option<Encoding>,
    content_type: Option<ContentType>,
    bytes: Bytes,
}

impl ScriptBody {
    pub fn bytes(&self) -> miette::Result<Vec<u8>> {
        match self.encoding() {
            Some(e) => e.decode(&self.bytes),
            None => Ok(self.bytes.to_vec()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn content_type(&self) -> Option<&ContentType> {
        self.content_type.as_ref()
    }

    pub fn encoding(&self) -> Option<&Encoding> {
        self.encoding.as_ref()
    }

    pub fn text(&self) -> miette::Result<String> {
        String::from_utf8(self.bytes()?).into_diagnostic()
    }

    pub fn json(&self) -> miette::Result<JsonValue> {
        serde_json::from_slice(&self.bytes()?).into_diagnostic()
    }
}

#[derive(Debug, Clone)]
pub struct ScriptResponse {
    pub status: u16,
    pub version: Version,
    pub url: String,
    pub remote_addr: Option<SocketAddr>,
    pub headers: Headers,
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
            .with_fn("json", |r: &mut Self| {
                r.body.json().map_err(|e| {
                    Box::new(EvalAltResult::ErrorRuntime(
                        format!("Unable to parse response body as json: {}", e).into(),
                        Position::NONE,
                    ))
                })
            })
            .with_fn("text", |r: &mut Self| {
                r.body.text().map_err(|e| {
                    Box::new(EvalAltResult::ErrorRuntime(
                        format!("Unable to parse response body as text: {}", e).into(),
                        Position::NONE,
                    ))
                })
            });
    }
}

impl ScriptResponse {
    pub fn from_response(res: Response) -> miette::Result<Self> {
        Ok(Self {
            status: res.status().as_u16(),
            version: res.version(),
            url: res.url().as_str().into(),
            remote_addr: res.remote_addr(),
            headers: Headers {
                headers: res.headers().clone(),
            },
            content_length: res.content_length(),
            body: {
                let encoding = res.headers().get(CONTENT_ENCODING);

                let encoding = if let Some(encoding) = encoding {
                    let encoding = encoding
                        .to_str()
                        .into_diagnostic()
                        .context("Unable to decode Content-Encoding header")?;
                    Some(Encoding::from_str(encoding))
                } else {
                    None
                };

                let content_type = res.headers().get(CONTENT_TYPE);

                let content_type = if let Some(content_type) = content_type {
                    let content_type = content_type
                        .to_str()
                        .into_diagnostic()
                        .context("Unable to decode Content-Encoding header")?;
                    Some(ContentType::from_str(content_type))
                } else {
                    None
                };

                ScriptBody {
                    encoding,
                    content_type,
                    bytes: res.bytes().into_diagnostic()?,
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

    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.status).expect("set from Response")
    }
}

#[derive(Clone)]
struct Variables {
    config: Rc<Config>,
    state: Rc<RefCell<State>>,
}

impl Variables {
    // These functions are needed since properties don't seem to fall back to the .with_indexer_*
    // functions.
    fn get_var(&mut self, idx: &str) -> Result<PersistedVariable, Box<EvalAltResult>> {
        self.state
            .borrow_mut()
            .variables
            .get(idx)
            .cloned()
            .ok_or_else(|| {
                Box::new(EvalAltResult::ErrorIndexNotFound(
                    idx.into(),
                    Position::NONE,
                ))
            })
    }

    fn set_var_reset_expire(&mut self, idx: &str, value: &str) {
        self.set_var(
            idx,
            PersistedVariable {
                value: value.into(),
                expires_at: self
                    .config
                    .get_variable(idx)
                    .unwrap()
                    .persist
                    .duration()
                    .map(|d| Utc::now() + d),
            },
        )
    }

    fn set_var(&mut self, idx: &str, value: PersistedVariable) {
        self.state.borrow_mut().variables.insert(idx.into(), value);
    }
}

impl CustomType for Variables {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Variables")
            .on_debug(|this: &mut Self| {
                use std::fmt::Write;
                let mut out = String::new();
                out += "Variables {";
                let vars = &this.state.borrow().variables;
                if !vars.is_empty() {
                    out += "\n";
                    for (name, var) in &this.state.borrow().variables {
                        write!(out, "    {}: ", name).expect("Write to string can't fail");
                        var.debug(&mut out).expect("Write to string can't fail");
                        writeln!(out, ",").expect("Write to string can't fail");
                    }
                }
                out += "}";
                out
            })
            .with_indexer_get(Variables::get_var)
            .with_indexer_set(Variables::set_var_reset_expire)
            .with_indexer_set(Variables::set_var);
    }
}

pub fn lookup_variable(
    name: &str,
    state: &RefCell<State>,
    vars: &HashMap<String, Interpolated<'static>>,
) -> Result<Option<Dynamic>, Box<EvalAltResult>> {
    if let Some(var) = state.borrow().variables.get(name) {
        Ok(Some(var.value.deref().into()))
    } else if let Some(var) = vars.get(name) {
        Ok(Some(var.interpolate(vars).unwrap().to_string().into()))
    } else {
        Ok(None)
    }
}

pub fn base_engine() -> Engine {
    let mut engine = Engine::new();

    // plugins
    RandomPackage::new().register_into_engine(&mut engine);
    FilesystemPackage::new().register_into_engine(&mut engine);

    // custom types
    engine
        .build_type::<ScriptResponse>()
        .build_type::<Variables>()
        .build_type::<PersistedVariable>()
        .build_type::<Headers>();

    // cookie type
    engine
        .register_fn("parse_cookie", |s: ImmutableString| {
            Cookie::from_str(&s).map_err(|e| {
                Box::new(EvalAltResult::ErrorSystem(
                    "Unable to parse cookie".into(),
                    Box::new(e),
                ))
            })
        })
        .register_get("name", |cookie: &mut Cookie| cookie.name().to_string())
        .register_get("value", |cookie: &mut Cookie| cookie.value().to_string())
        .register_get("expires_at", |cookie: &mut Cookie| {
            cookie
                .expires_datetime()
                .map(|d| DateTime::from_timestamp(d.unix_timestamp(), 0).unwrap())
        });

    engine
}

pub fn script_engine(
    state: Rc<RefCell<State>>,
    config: Rc<Config>,
    vars: Rc<HashMap<String, Interpolated<'static>>>,
) -> (Engine, Scope<'static>) {
    let mut engine = base_engine();

    #[allow(deprecated, reason = "Volatile, but we need it")]
    engine.on_var({
        let state = state.clone();
        let vars = vars.clone();
        move |name, _index, _ctx| lookup_variable(name, &state, &vars)
    });

    let mut scope = Scope::new();

    scope.push("vars", Variables { config, state });

    (engine, scope)
}
