use std::{cell::RefCell, collections::HashMap, net::SocketAddr, ops::Deref, rc::Rc, str::FromStr};

use chrono::{DateTime, Utc};
use cookie::Cookie;
use miette::IntoDiagnostic;
use reqwest::{
    StatusCode, Version,
    blocking::Response,
    header::{self, HeaderMap},
};
use rhai::{
    CustomType, Dynamic, Engine, EvalAltResult, ImmutableString, Position, Scope, TypeBuilder,
};

use crate::{
    config::Config,
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
            headers: Headers {
                headers: res.headers().clone(),
            },
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

pub fn script_engine(
    state: Rc<RefCell<State>>,
    config: Rc<Config>,
    vars: Rc<HashMap<String, Interpolated<'static>>>,
) -> (Engine, Scope<'static>) {
    let mut engine = Engine::new();

    engine
        .build_type::<ScriptResponse>()
        .build_type::<Variables>()
        .build_type::<PersistedVariable>()
        .build_type::<Headers>()
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
