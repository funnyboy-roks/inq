use std::{ops::Deref, rc::Rc, str::FromStr, time::Instant};

use inq_lang::{
    IStr, Route,
    eval::{EvalError, Scope, Special, value::ValueRef},
};
use miette::{IntoDiagnostic, bail};
use reqwest::Url;

use crate::{
    cli::RouteCommand,
    config::Config,
    print::{print_request, print_response},
    script::{request::RequestValue, response::ResponseValue, url::UrlValue},
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
        .expect("At least one route is defined")
        .max("Name".len());
    let method_len = config
        .routes
        .iter()
        .map(|r| r.method.as_str().len())
        .max()
        .expect("At least one route is defined")
        .max("Method".len());
    let args = config
        .routes
        .iter()
        .map(|r| {
            format!(
                "{}",
                r.args
                    .iter()
                    .map(|a| a.name.as_istr().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>();

    let args_len = args.iter().map(|a| a.len()).max().unwrap_or_default() + 2;

    {
        use owo_colors::OwoColorize as _;
        println!(
            "{:<name_len$}{}  {:<method_len$}  {}",
            "Name".blue().bold(),
            if args_len != 0 {
                format!("({})", "Args".magenta())
            } else {
                String::new()
            },
            "Method".yellow().bold(),
            "Endpoint".green().bold(),
        );
        for (route, args) in config.routes.iter().zip(args) {
            print!("{:>name_len$}", route.name.to_string().blue(),);
            if route.args.is_empty() {
                print!("{:>args_len$}", "");
            } else {
                print!("({})", args.magenta());
            }
            print!(
                "  {:<method_len$}  {}",
                route.method.as_str().yellow(),
                route.endpoint.green()
            );
            println!();
        }
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
    } else if let Some(base_url_lazy) = config.engine.global().get_lazy_variable("BASE_URL")? {
        let span = base_url_lazy.value_span();
        let mut full_url = base_url_lazy.get()?.expect_downcast::<UrlValue>(span)?.0;
        let (path, query) = url
            .split_once('?')
            .map(|(p, q)| (p, Some(q)))
            .unwrap_or((&url, None));
        full_url.set_path(path);
        full_url.set_query(query);
        full_url
    } else {
        miette::bail! {
            labels = vec![route.endpoint.span.with_label("here")],
            "URL scheme or BASE_URL must be speicifed",
        }
    };
    Ok(url)
}

pub fn run(route_cmd: RouteCommand, config: Config, state: &mut State) -> miette::Result<()> {
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
        let value: ValueRef = if let Some(value) = value {
            IStr::from(value.clone()).into()
        } else if let Some(ref default_value) = arg.default_value {
            scope.eval(default_value.clone())?
        } else {
            return Err(EvalError::Custom {
                message: format!("Argument `{}` is required", arg.name),
                span: arg.name.span,
            }
            .into());
        };
        scope.set_variable_ref(arg.name.as_ref(), value, true);
    }

    let url = parse_url(&config, route, &scope)?;

    let request = Rc::new(RequestValue::new(
        route_cmd.client,
        route.method.to_reqwest(),
        url,
    ));

    if let Some(ref before) = route.before {
        let scope = scope.make_child();
        scope.set_special(Special::Request(ValueRef::from_ref(request.clone())));

        scope.eval(before.clone().into())?;
    }

    let req_body = request.body.borrow().clone();
    let (request, client) = request.deref().clone().into_reqwest();

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
