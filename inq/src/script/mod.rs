use std::rc::Rc;

use inq_lang::{
    IStr, StringExt,
    eval::{Engine, EvalResult, Special, value::ValueRef},
};
use miette::IntoDiagnostic;
use rustyline::DefaultEditor;

use crate::script::{
    bytes_value::BytesValue, client::ClientConfig, cookie::CookieValue, datetime::DateTimeValue,
    duration::DurationValue, header::HeaderMapValue, json::Json, request::RequestValue,
    response::ResponseValue, url::UrlValue,
};

pub(crate) mod bytes_value;
pub(crate) mod client;
pub(crate) mod cookie;
pub(crate) mod datetime;
pub(crate) mod duration;
pub(crate) mod faker;
pub(crate) mod header;
pub(crate) mod json;
pub(crate) mod request;
pub(crate) mod response;
pub(crate) mod url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBody {
    pub(crate) content_type: Option<String>,
    pub(crate) bytes: Vec<u8>,
}

impl ScriptBody {
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn text(&self) -> miette::Result<&str> {
        str::from_utf8(&self.bytes).into_diagnostic()
    }

    pub fn json(&self) -> EvalResult<Json> {
        Ok(Json(serde_json::from_slice(&self.bytes).into_diagnostic()?))
    }
}

fn prompt(prompt: &str) -> miette::Result<String> {
    let mut rl = DefaultEditor::new().into_diagnostic()?;
    use owo_colors::OwoColorize as _;
    rl.readline(
        &(
            // format!("{}: ", prompt),
            format!("{}: ", prompt.blue().bold())
        ),
    )
    .into_diagnostic()
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
    engine.register_type::<faker::FakerValue>();

    let global = engine.global();

    global.declare_function("env", |_ctx, var: IStr| {
        std::env::var(&*var)
            .ok()
            .map(IStr::from)
            .map(ValueRef::new)
            .unwrap_or_else(ValueRef::null)
    });
    global.declare_function("prompt", |ctx, text: IStr| {
        let e = prompt(&text).map_err(|e| ctx.error(format!("Error prompting: {}", e)))?;
        Ok(e.intern())
    });

    global.declare_function("json", Json::from_value);
    global.declare_function("assert", |ctx, b: bool| {
        if b {
            Ok(ValueRef::null())
        } else {
            Err(ctx.error_args("Assertion failed", [(0, "Expected to be true")]))
        }
    });
    global.declare_function("assert_success", |ctx, ()| match ctx.scope().special() {
        None | Some(Special::None) | Some(Special::Request(_)) => {
            Err(ctx.error("`assert_success` may only be used in after block"))
        }
        Some(Special::Response(r)) => {
            let res = r.downcast_rc::<ResponseValue>().expect("we set this type");
            if res.status().is_success() {
                Ok(())
            } else {
                Err(ctx.error(format!("Expected successful status, got {}", res.status())))
            }
        }
    });

    global.declare_function("read_text_file", |ctx, s: IStr| {
        std::fs::read_to_string(&*s)
            .map_err(|e| {
                ctx.error_args(
                    format!("Unable to read text file: {}", e),
                    [(0, "this path")],
                )
            })
            .map(IStr::from)
    });
    global.declare_function("read_raw_file", |ctx, s: IStr| {
        std::fs::read(&*s)
            .map_err(|e| {
                ctx.error_args(
                    format!("Unable to read raw file: {}", e),
                    [(0, "this path")],
                )
            })
            .map(BytesValue::from)
    });

    engine
}
