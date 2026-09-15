use std::{cell::RefCell, rc::Rc, str::FromStr, string::String};

use inq_lang::{
    IStr,
    eval::{
        Engine, EvalError, EvalResult, Special,
        registry::{FunctionValue, Registry},
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Float, Int, Null, Object},
        },
    },
    lex::Lexer,
    parse::{Attribute, Ident, Item, Parser},
};
use miette::{IntoDiagnostic, LabeledSpan, NamedSource};
use reqwest::{
    Url,
    blocking::{Body, Client, Request},
    header::{self, HeaderMap, HeaderName, HeaderValue},
};

use crate::{
    cli::{Cli, ParseCommand},
    state::State,
};

fn json(ctx: &CallContext, arg: ValueRef, out: &mut String) -> EvalResult<()> {
    // SAFETY: we only write valid strings
    let outw = unsafe { out.as_mut_vec() };
    #[allow(clippy::redundant_pattern_matching)]
    if let Some(s) = arg.borrow().downcast_ref::<IStr>() {
        serde_json::to_writer(outw, &**s).unwrap();
    } else if let Some(i) = arg.downcast::<Int>() {
        serde_json::to_writer(outw, &i).unwrap();
    } else if let Some(f) = arg.downcast::<Float>() {
        serde_json::to_writer(outw, &f).unwrap();
    } else if let Some(b) = arg.downcast::<bool>() {
        serde_json::to_writer(outw, &b).unwrap();
    } else if let Some(_) = arg.downcast::<Null>() {
        serde_json::to_writer(outw, &()).unwrap();
    } else if let Some(a) = arg.borrow().downcast_ref::<Array>() {
        out.push('[');
        for (i, a) in a.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            json(ctx, a.clone(), out)?;
        }
        out.push(']');
    } else if let Some(o) = arg.borrow().downcast_ref::<Object>() {
        out.push('{');
        for (i, (k, v)) in o.0.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            // SAFETY: we only write valid strings
            let outw = unsafe { out.as_mut_vec() };
            serde_json::to_writer(outw, &**k).unwrap();
            out.push(':');
            json(ctx, v.clone(), out)?;
        }
        out.push('}');
    } else {
        return Err(EvalError::Custom {
            message: format!("Invalid type for JSON: {}", arg.type_name_of()),
            span: ctx.span(),
        });
    }
    Ok(())
}

fn env(s: &IStr) -> ValueRef {
    std::env::var(&**s)
        .ok()
        .map(IStr::from)
        .map(ValueRef::new)
        .unwrap_or_else(ValueRef::null)
}

fn make_engine() -> Rc<Engine> {
    let engine = Engine::new();

    engine.register_type::<RequestValue>();

    let global = engine.global();

    global.set_variable("env", env as fn(&IStr) -> ValueRef, true);
    global.set_variable(
        "print",
        FunctionValue::new(|_ctx, s: ValueRef| {
            let mut out = String::new();
            s.borrow().to_string(&mut out);
            println!("{}", out);
            ValueRef::null()
        }),
        true,
    );
    global.set_variable(
        "debug",
        FunctionValue::new(|_ctx, s: ValueRef| {
            println!("{:#?}", s.debug());
            ValueRef::null()
        }),
        true,
    );

    global.set_variable(
        "json",
        FunctionValue::new(|ctx, arg| {
            let mut out = String::new();
            json(&ctx, arg, &mut out)?;
            Ok(Json(out.into()))
        }),
        true,
    );

    engine
}

#[derive(Debug, Clone)]
struct Json(IStr);
impl Value for Json {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Json".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str(&self.0)
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }

    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
    }
}

#[derive(Debug)]
struct Config {
    routes: Vec<inq_lang::parse::Route>,
    persisted_vars: Vec<Ident>,
    engine: Rc<Engine>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            routes: Default::default(),
            persisted_vars: Default::default(),
            engine: make_engine(),
        }
    }
}

fn map_method(method: inq_lang::lex::Method) -> reqwest::Method {
    match method {
        inq_lang::lex::Method::Get => reqwest::Method::GET,
        inq_lang::lex::Method::Head => reqwest::Method::HEAD,
        inq_lang::lex::Method::Post => reqwest::Method::POST,
        inq_lang::lex::Method::Put => reqwest::Method::PUT,
        inq_lang::lex::Method::Delete => reqwest::Method::DELETE,
        inq_lang::lex::Method::Options => reqwest::Method::OPTIONS,
        inq_lang::lex::Method::Trace => reqwest::Method::TRACE,
        inq_lang::lex::Method::Patch => reqwest::Method::PATCH,
    }
}

#[derive(Debug, Clone)]
struct HeaderMapValue(HeaderMap);
impl Value for HeaderMapValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "HeaderMap".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{:?}", self).unwrap();
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <HeaderMap as std::fmt::Debug>::fmt(&self.0, fmt)
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_index_get_set(
            |ctx, this, s: &IStr| {
                let name = HeaderName::from_str(s.as_str()).map_err(|_| EvalError::Custom {
                    message: "Invalid header name".into(),
                    span: ctx.index_span,
                })?;
                Ok(this
                    .0
                    .get(name)
                    .map(HeaderValue::to_str)
                    .transpose()
                    .unwrap()
                    .map(IStr::from))
            },
            |ctx, this, s: &IStr, v| {
                let val = v.expect_downcast::<IStr>(ctx.rhs_span)?;
                let name = HeaderName::from_str(s.as_str()).map_err(|_| EvalError::Custom {
                    message: "Invalid header name".into(),
                    span: ctx.index_span,
                })?;
                let value = HeaderValue::from_str(&val).map_err(|_| EvalError::Custom {
                    message: "Invalid header value".into(),
                    span: ctx.rhs_span,
                })?;
                this.0.insert(name, value);
                Ok(())
            },
        );
    }
}

#[derive(Debug, Clone)]
enum RequestBody {
    None,
    Json(Json),
    Text(IStr),
}

#[derive(Debug, Clone)]
struct RequestValue {
    method: reqwest::Method,
    url: Rc<RefCell<UrlValue>>,
    headers: Rc<RefCell<HeaderMapValue>>,
    body: RequestBody,
}

impl From<RequestValue> for Request {
    fn from(value: RequestValue) -> Self {
        let mut request = Request::new(value.method, value.url.borrow().0.clone());
        *request.headers_mut() = value.headers.borrow().0.clone();
        match value.body {
            RequestBody::None => {}
            RequestBody::Json(json) => {
                *request.body_mut() = Some(Body::from(json.0.as_str().to_string()));
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
        write!(out, "{} {}", self.method, self.url.borrow().0).unwrap();
    }
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, f)
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.engine().register_type::<UrlValue>();
        registry.engine().register_type::<HeaderMapValue>();

        registry.register_field_get("url", |_, this| ValueRef::from_ref(this.url.clone()));
        registry.register_field_get("headers", |_, this| {
            ValueRef::from_ref(this.headers.clone())
        });
        registry.register_field_get_set(
            "body",
            |_, this| match &this.body {
                RequestBody::None => ValueRef::null(),
                RequestBody::Json(json) => json.clone().into(),
                RequestBody::Text(text) => text.clone().into(),
            },
            |ctx, this, rhs| {
                if rhs.is::<Null>() {
                    this.body = RequestBody::None;
                    this.headers.borrow_mut().0.remove(header::CONTENT_TYPE);
                } else if let Some(json) = rhs.downcast::<Json>() {
                    this.body = RequestBody::Json(json);
                    this.headers.borrow_mut().0.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_str("application/json").unwrap(),
                    );
                } else if let Some(text) = rhs.downcast::<IStr>() {
                    this.body = RequestBody::Text(text);
                    this.headers.borrow_mut().0.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_str("text/plain; charset=utf-8").unwrap(),
                    );
                } else {
                    return Err(EvalError::Custom {
                        message: "Invalid value for body.  Must be `Json` or `String`".into(),
                        span: ctx.span(),
                    });
                }
                Ok(())
            },
        );
    }
}

#[derive(Debug, Clone)]
struct UrlValue(Url);
impl Value for UrlValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Url".into()
    }
    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }
    fn truthy(&self) -> bool {
        true
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self.0).unwrap();
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        macro_rules! debug {
            ($e: expr) => {
                std::fmt::from_fn(|fmt| ValueRef::from($e).borrow().debug(fmt))
            };
        }
        fmt.debug_struct("Url")
            .field("host", &debug!(self.0.host_str().map(IStr::from)))
            .field("domain", &debug!(self.0.domain().map(IStr::from)))
            .field("port", &debug!(self.0.port().map(Int::from)))
            .field("path", &debug!(IStr::from(self.0.path())))
            .field("query", &debug!(self.0.query().map(IStr::from)))
            .field("fragment", &debug!(self.0.fragment().map(IStr::from)))
            .field("authority", &debug!(IStr::from(self.0.authority())))
            .field("username", &debug!(IStr::from(self.0.username())))
            .field("password", &debug!(self.0.password().map(IStr::from)))
            .finish()
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("host", |_, this| this.0.host_str().map(IStr::from));
        registry.register_field_get("domain", |_, this| this.0.domain().map(IStr::from));
        registry.register_field_get("port", |_, this| this.0.port().map(Int::from));
        registry.register_field_get::<IStr>("path", |_, this| this.0.path().into());
        registry.register_field_get("query", |_, this| this.0.query().map(IStr::from));
        registry.register_field_get("fragment", |_, this| this.0.fragment().map(IStr::from));
        registry.register_field_get::<IStr>("authority", |_, this| this.0.authority().into());
        registry.register_field_get::<IStr>("username", |_, this| this.0.username().into());
        registry.register_field_get("password", |_, this| this.0.password().map(IStr::from));
    }
}

pub(crate) fn run(
    _cli: &Cli,
    cmd: &ParseCommand,
    _config: crate::config::Config,
    _state: Rc<RefCell<State>>,
) -> miette::Result<()> {
    let file_name = cmd.file.file_name().unwrap().to_string_lossy();
    let content = std::fs::read_to_string(&cmd.file).into_diagnostic()?;
    let lex = Lexer::new(&content);

    let mut parser = Parser::new(lex).map_err(|e| {
        miette::Report::from(e).with_source_code(NamedSource::new(&file_name, content.to_string()))
    })?;

    let mut config = Config::default();
    while let Some(item) = parser.take_item().map_err(|e| {
        miette::Report::from(e).with_source_code(NamedSource::new(&file_name, content.to_string()))
    })? {
        match item {
            Item::Variable(v) => {
                if v.attributes.contains(&Attribute::Persist) {
                    config.persisted_vars.push(v.name.clone());
                }
                config.engine.global().add_variable(v);
            }
            Item::Route(r) => config.routes.push(r),
        }
    }

    let route = config
        .routes
        .into_iter()
        .find(|r| r.name == "notifications/new")
        .unwrap();

    let eval_scope = config.engine.global().make_child();

    for (arg, value) in route
        .args
        .into_iter()
        .zip(cmd.args.iter().map(Some).chain(std::iter::repeat(None)))
    {
        let value = if let Some(value) = value {
            value.clone()
        } else if let Some(default_value) = arg.default_value {
            let span = default_value.span;
            eval_scope
                .eval(default_value)?
                .expect_downcast::<IStr>(span)?
                .as_str()
                .into()
        } else {
            return Err(
                miette::miette! {
                    labels = vec![LabeledSpan::new_with_span(Some("this argument".into()), arg.name.span,)],
                    "Argument `{}` is required",
                    arg.name
                }
                .with_source_code(NamedSource::new(&file_name, content.to_string()))
            );
        };

        eval_scope.set_variable(arg.name.as_ref(), IStr::from(value), true);
    }

    let endpoint_span = route.endpoint.span;
    let endpoint = eval_scope
        .eval(route.endpoint)?
        .expect_downcast::<IStr>(endpoint_span)?;
    let mut request = Request::new(map_method(route.method), Url::from_str(&endpoint).unwrap());
    if let Some(before) = route.before {
        let request_value = Rc::new(RefCell::new(RequestValue {
            method: request.method().clone(),
            url: Rc::new(RefCell::new(UrlValue(request.url().clone()))),
            headers: Rc::new(RefCell::new(HeaderMapValue(Default::default()))),
            body: RequestBody::None,
        }));
        let scope = eval_scope.make_child();
        scope.set_special(Special::Request(ValueRef::from_ref(request_value.clone())));
        scope.eval(before.into()).map_err(|e| {
            miette::Report::from(e)
                .with_source_code(NamedSource::new(&file_name, content.to_string()))
        })?;

        request = request_value.borrow().clone().into();
    }

    let client = Client::builder()
        //
        .build()
        .unwrap();

    dbg!(&request);

    let res = client.execute(request).unwrap();

    dbg!(res);

    Ok(())
}

#[cfg(test)]
mod test {
    use miette::NamedSource;

    use crate::cli::parse::Json;

    use super::make_engine;
    use inq_lang::{IStr, lex::Lexer, parse::Parser};

    macro_rules! eval {
        ($engine: expr, $($tt: tt)*) => {{
            let content = stringify!($($tt)*);
            let lex = Lexer::new(&content);
            let mut parser = Parser::new(lex)
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap();
            let expr = parser
                .take_expr()
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap()
                .unwrap();

            $engine.global().eval(expr)
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap()
        }};
    }

    #[test]
    fn valid_json() {
        let e = make_engine();
        let j = eval!(e, json({
            "key1": "bar",
            "key2": 0,
            "key3": 0.5,
            "key4": true,
            "key5": false,
            "key6": null,
            "key7": {},
            "key8": { "key1": 0 },
            "key9": { "key1": { "key1": { "key1": 0 } } },
            "key10": [],
            "key11": [0],
            "key12": [{ "key1": { "key1": { "key1": 0 } } }],
            "key13": [[0]],
            "key14": [0, ["hello"], { "foo": "bar" }],
        }));

        assert_eq!(
            j.assert_downcast::<Json>().0.as_str(),
            serde_json::to_string(&serde_json::json!({
                "key1": "bar",
                "key2": 0,
                "key3": 0.5,
                "key4": true,
                "key5": false,
                "key6": null,
                "key7": {},
                "key8": { "key1": 0 },
                "key9": { "key1": { "key1": { "key1": 0 } } },
                "key10": [],
                "key11": [0],
                "key12": [{ "key1": { "key1": { "key1": 0 } } }],
                "key13": [[0]],
                "key14": [0, ["hello"], { "foo": "bar" }],
            }))
            .unwrap()
        )
    }

    #[test]
    fn valid_json_unquoted() {
        let e = make_engine();
        let j = eval!(e, json({
                key1: "bar",
                key2: 0,
                key3: 0.5,
                key4: true,
                key5: false,
                key6: null,
                key7: {},
                key8: { key1: 0 },
                key9: { key1: { key1: { key1: 0 } } },
                key10: [],
                key11: [0],
                key12: [{ key1: { key1: { key1: 0 } } }],
                key13: [[0]],
                key14: [0, ["hello"], { foo: "bar" }],
        }));

        assert_eq!(
            j.assert_downcast::<Json>().0.as_str(),
            serde_json::to_string(&serde_json::json!({
                "key1": "bar",
                "key2": 0,
                "key3": 0.5,
                "key4": true,
                "key5": false,
                "key6": null,
                "key7": {},
                "key8": { "key1": 0 },
                "key9": { "key1": { "key1": { "key1": 0 } } },
                "key10": [],
                "key11": [0],
                "key12": [{ "key1": { "key1": { "key1": 0 } } }],
                "key13": [[0]],
                "key14": [0, ["hello"], { "foo": "bar" }],
            }))
            .unwrap()
        )
    }
}
