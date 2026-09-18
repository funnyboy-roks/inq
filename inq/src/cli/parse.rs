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

    let mut config = Self {
        routes: Default::default(),
        persisted_vars: Default::default(),
        engine: make_engine(),
    };
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
