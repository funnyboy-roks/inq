use std::{cell::RefCell, rc::Rc, str::FromStr, time::Instant};

use inq_lang::{
    IStr, Route,
    eval::{EvalError, Scope, Special, value::ValueRef},
};
use miette::{IntoDiagnostic, bail};
use reqwest::{Url, blocking::Request};

use crate::{
    cli::{Cli, RouteCommand},
    config::Config,
    print::{print_request, print_response},
    script::{request::RequestValue, response::ResponseValue},
    state::State,
    util::ToReqwest,
};

fn list_routes(config: Config) -> miette::Result<()> {
    if config.routes.is_empty() {
        bail!("No routes defined");
    }

    let name_len = config
        .routes
        .iter()
        .map(|r| r.name.to_string().len())
        .max()
        .expect("At least one route is defined");
    let method_len = config
        .routes
        .iter()
        .map(|r| r.method.as_str().len())
        .max()
        .expect("At least one route is defined");

    for route in &config.routes {
        use owo_colors::OwoColorize as _;
        println!(
            "{:<name_len$}  {:<method_len$}  {}",
            route.name.to_string().blue().bold(),
            route.method.as_str().yellow(),
            route.endpoint.green()
        );
    }

    Ok(())
}

fn parse_url(config: &Config, route: &Route, scope: &Rc<Scope>) -> Result<Url, miette::Error> {
    let url = route.endpoint_eval(scope)?;
    let url = if let Some((scheme, _)) = url.split_once("://")
        && scheme.chars().all(|c| c.is_ascii_alphabetic())
    {
        Url::from_str(&url).map_err(|e| {
            miette::miette! {
                labels = vec![route.endpoint.span.with_label("here")],
                "Unable to parse url: {}", e
            }
        })?
    } else if let Some(base_url) = config.engine.global().get_variable("BASE_URL")? {
        if let Some(base_url) = base_url.downcast::<IStr>() {
            let string = format!("{}{}", base_url, url);
            Url::from_str(&string).map_err(|e| {
                miette::miette! {
                    labels = vec![route.endpoint.span.with_label("here")],
                    "Unable to parse url: {}", e
                }
            })?
        } else {
            // TODO: span
            bail!("BASE_URL must be a string");
        }
    } else {
        miette::bail! {
            labels = vec![route.endpoint.span.with_label("here")],
            "URL scheme or BASE_URL must be speicifed",
        }
    };
    Ok(url)
}

pub(crate) fn run(
    _: &Cli,
    route_cmd: &RouteCommand,
    config: Config,
    state: &mut State,
) -> miette::Result<()> {
    let Some(route) = &route_cmd.route else {
        return list_routes(config);
    };

    let route = config.expect_route(route)?;
    let scope = config.engine.global().make_child();

    for (arg, value) in route.args.iter().zip(
        route_cmd
            .args
            .iter()
            .map(Some)
            .chain(std::iter::repeat(None)),
    ) {
        let value = if let Some(value) = value {
            value.clone()
        } else if let Some(ref default_value) = arg.default_value {
            let span = default_value.span;
            scope
                .eval(default_value.clone())?
                .expect_downcast::<IStr>(span)?
                .as_str()
                .into()
        } else {
            return Err(EvalError::Custom {
                message: format!("Argument `{}` is required", arg.name),
                span: arg.name.span,
            }
            .into());
        };

        scope.set_variable(arg.name.as_ref(), IStr::from(value), true);
    }

    let url = parse_url(&config, route, &scope)?;

    let request = Rc::new(RefCell::new(RequestValue::new(
        route.method.to_reqwest(),
        url,
    )));

    if let Some(ref before) = route.before {
        let scope = scope.make_child();
        scope.set_special(Special::Request(ValueRef::from_ref(request.clone())));

        scope.eval(before.clone().into())?;
    }

    let req_body = request.borrow().body.clone();
    let request: Request = request.borrow().clone().into();

    let client = config.client()?;

    print_request(&request, &req_body)?;

    let start = Instant::now();
    let res = client.execute(request).into_diagnostic()?;
    let elapsed = start.elapsed();

    let res = ResponseValue::try_from(res)?;

    print_response(&res, elapsed, route_cmd.raw)?;

    if let Some(ref after) = route.after {
        let scope = scope.make_child();
        scope.set_special(Special::Response(ValueRef::new(res)));

        scope.eval(after.clone().into())?;
    }

    state.update_variables(&config)?;

    Ok(())
}
