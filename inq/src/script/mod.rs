use std::{fmt::Display, rc::Rc};

use bytes::Bytes;
use inq_lang::{
    IStr,
    eval::{Engine, EvalResult, registry::FunctionValue, value::ValueRef},
};
use miette::{IntoDiagnostic, bail};

use crate::{
    decode::{decode_brotli, decode_deflate, decode_gzip, decode_zstd},
    script::{
        bytes_value::BytesValue, client::ClientConfig, cookie::CookieValue,
        datetime::DateTimeValue, duration::DurationValue, header::HeaderMapValue, json::Json,
        request::RequestValue, response::ResponseValue, url::UrlValue,
    },
};

pub(crate) mod bytes_value;
pub(crate) mod client;
pub(crate) mod cookie;
pub(crate) mod datetime;
pub(crate) mod duration;
pub(crate) mod header;
pub(crate) mod json;
pub(crate) mod request;
pub(crate) mod response;
pub(crate) mod url;

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
            Encoding::Unknown(e) => {
                bail!("Unknown encoding: {}", e);
            }
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
pub struct ScriptBody {
    pub(crate) encoding: Option<Encoding>,
    pub(crate) content_type: Option<String>,
    pub(crate) bytes: Bytes,
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

    pub fn encoding(&self) -> Option<&Encoding> {
        self.encoding.as_ref()
    }

    pub fn text(&self) -> miette::Result<String> {
        String::from_utf8(self.bytes()?).into_diagnostic()
    }

    pub fn json(&self) -> EvalResult<Json> {
        Ok(Json(
            serde_json::from_slice(&self.bytes()?).into_diagnostic()?,
        ))
    }
}

pub fn base_engine() -> Rc<Engine> {
    let engine = Engine::new();

    engine.register_type::<RequestValue>();
    engine.register_type::<ResponseValue>();
    engine.register_type::<UrlValue>();
    engine.register_type::<HeaderMapValue>();
    engine.register_type::<ClientConfig>();

    engine.register_type::<Json>();
    engine.register_type::<BytesValue>();
    engine.register_type::<DurationValue>();
    engine.register_type::<CookieValue>();
    engine.register_type::<DateTimeValue>();

    // plugins
    // RandomPackage::new().register_into_engine(&mut engine);
    // FilesystemPackage::new().register_into_engine(&mut engine);

    let global = engine.global();

    fn env(s: &IStr) -> ValueRef {
        std::env::var(&**s)
            .ok()
            .map(IStr::from)
            .map(ValueRef::new)
            .unwrap_or_else(ValueRef::null)
    }
    global.set_variable("env", env as fn(&IStr) -> ValueRef, true);

    global.set_variable(
        "print",
        FunctionValue::new(|_ctx, s: ValueRef| {
            let mut out = String::new();
            s.value().to_string(&mut out);
            println!("{}", out);
        }),
        true,
    );
    global.set_variable(
        "debug",
        FunctionValue::new(|_ctx, s: ValueRef| {
            eprintln!("{:#?}", s.debug());
        }),
        true,
    );

    global.set_variable("json", FunctionValue::new(Json::from_value), true);
    global.set_variable(
        "assert",
        FunctionValue::new(|ctx, b: bool| {
            if b {
                Ok(ValueRef::null())
            } else {
                Err(ctx.error("Assertion failed"))
            }
        }),
        true,
    );

    engine
}
