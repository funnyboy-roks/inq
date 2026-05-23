use std::{
    cell::RefCell, collections::BTreeMap, fmt::Display, io::Write, ops::Deref, rc::Rc,
    str::FromStr, time::Instant,
};

use anstream::{eprintln, println};
use chrono::{DateTime, Utc};
use clap::Parser;
use cookie::Cookie;
use miette::{Context, IntoDiagnostic, bail};
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue},
};
use rhai::{Engine, EvalAltResult, ImmutableString, Position, Scope};
use serde_json::Value as JsonValue;

use crate::{
    cli::{Cli, QueryCommand, SubCmd, VariableCommand},
    config::Config,
    script::{ScriptBody, ScriptResponse},
    state::{PersistedVariable, State},
};

mod cli;
mod config;
mod script;
mod state;
mod util;

fn pretty_print_json(w: &mut impl Write, json: JsonValue, indent: usize) -> std::io::Result<()> {
    use owo_colors::OwoColorize as _;

    fn apply_indent(w: &mut impl Write, indent: usize) -> std::io::Result<()> {
        write!(w, "\n{0:>1$}", "", indent * 4)
    }

    match json {
        JsonValue::Null => write!(w, "{}", "null".bright_black()),
        JsonValue::Bool(b) => write!(w, "{}", b.cyan()),
        JsonValue::Number(number) => write!(w, "{}", number.yellow()),
        JsonValue::String(_) => write!(w, "{}", json.green()),
        JsonValue::Array(values) => {
            write!(w, "[")?;
            if !values.is_empty() {
                for (i, v) in values.into_iter().enumerate() {
                    if i > 0 {
                        write!(w, ",")?;
                    }
                    apply_indent(w, indent + 1)?;
                    pretty_print_json(w, v, indent + 1)?;
                }
                apply_indent(w, indent)?;
            }
            write!(w, "]")
        }
        JsonValue::Object(map) => {
            write!(w, "{{")?;
            if !map.is_empty() {
                for (i, (k, v)) in map.into_iter().enumerate() {
                    if i > 0 {
                        write!(w, ",")?;
                    }
                    apply_indent(w, indent + 1)?;
                    write!(w, "{}: ", JsonValue::String(k).blue().bold())?;
                    pretty_print_json(w, v, indent + 1)?;
                }
                apply_indent(w, indent)?;
            }
            write!(w, "}}")
        }
    }
}

fn run_query(
    cli: &Cli,
    query_cmd: &QueryCommand,
    config: Rc<Config>,
    state: Rc<RefCell<State>>,
) -> miette::Result<()> {
    let client = Client::new();

    let query = config
        .get_query(&query_cmd.query)?
        .context("Query not defined")?;

    let vars = config.load_variables(&cli.subcmd, &state.borrow())?;

    for (name, val) in &vars {
        let mut state = state.borrow_mut();
        if let Some(var) = config.get_variable(name)
            && var.persist.persists()
            && !state.variables.contains_key(name)
        {
            state.variables.insert(
                name.to_string(),
                state::PersistedVariable {
                    value: val.interpolate(&vars)?.into_owned(),
                    expires_at: var.persist.duration().map(|d| Utc::now() + d),
                },
            );
        }
    }

    let (req, req_body) = query.to_request(&client, &vars)?;

    {
        use owo_colors::OwoColorize as _;

        eprintln!("{}:", "Request Details".cyan());

        eprintln!(
            "  {}:   {:?}",
            "HTTP Version".blue(),
            req.version().yellow()
        );
        eprintln!("  {}:         {}", "Method".blue(), req.method().yellow());
        eprintln!("  {}:            {}", "URL".blue(), req.url().yellow());
        if let Some(timeout) = req.timeout() {
            eprintln!("  {}:            {:?}", "Timeout".blue(), timeout.yellow());
        }

        eprintln!("  {}:", "Headers".blue());
        for (name, value) in req.headers() {
            match value.to_str() {
                Ok(s) => eprintln!("    {}: {}", name.yellow(), s),
                Err(_) => eprintln!("    {}: {:?}", name.yellow(), value),
            }
        }

        if let Some(req_body) = req_body {
            match req_body {
                config::PopulatedBody::Json(value) => {
                    eprintln!("{}:", "Request Body (JSON)".cyan());
                    pretty_print_json(&mut anstream::stderr().lock(), value, 0)
                        .into_diagnostic()?;
                    eprintln!();
                }
                config::PopulatedBody::Text(cow) => {
                    eprintln!("{}:", "Request Body (Raw)".cyan());

                    eprintln!("{}", cow);
                }
            }
        }

        eprintln!("{}", "Sending Request...".purple());
    }

    let start = Instant::now();
    let res = client.execute(req).into_diagnostic()?;
    let res = ScriptResponse::from_response(res, query_cmd.raw)?;
    let elapsed = start.elapsed();

    {
        use owo_colors::OwoColorize as _;

        eprintln!("{}:", "Response Details".cyan());
        let status = res.status();
        let status: &dyn Display = if status.is_server_error() {
            &status.bright_red()
        } else if status.is_client_error() {
            &status.red()
        } else if status.is_informational() {
            &status.blue()
        } else if status.is_redirection() {
            &status.yellow()
        } else if status.is_success() {
            &status.green()
        } else {
            &status
        };
        eprintln!("  {}:   {:?}", "HTTP Version".blue(), res.version.yellow());
        eprintln!("  {}:            {}", "URL".blue(), res.url.yellow());
        eprintln!("  {}:       {:?}", "Duration".blue(), elapsed.yellow());
        eprintln!("  {}:    {}", "Status Code".blue(), status);
        if let Some(remote_addr) = res.remote_addr {
            eprintln!("  {}: {}", "Remote Address".blue(), remote_addr.yellow());
        }

        eprintln!("  {}:", "Headers".blue());
        for (name, value) in &res.headers {
            match value.to_str() {
                Ok(s) => eprintln!("    {}: {}", name.yellow(), s),
                Err(_) => eprintln!("    {}: {:?}", name.yellow(), value),
            }
        }

        if let Some(content_len) = res.content_length
            && content_len != 0
        {
            match &res.body {
                ScriptBody::Text(t) => {
                    eprintln!("{}:", "Response Body (Raw)".cyan());
                    std::io::stdout()
                        .write_all(t.as_bytes())
                        .into_diagnostic()?;
                }
                ScriptBody::Json(json) => {
                    eprintln!("{}:", "Response Body (JSON)".cyan());
                    pretty_print_json(&mut anstream::stdout().lock(), json.clone(), 0)
                        .into_diagnostic()?;
                    println!();
                }
            }
        }
    }

    if let Some(post_script) = query.post_script {
        eprintln!(
            "{}",
            owo_colors::OwoColorize::purple(&"Running post-script")
        );

        let mut engine = Engine::new();

        #[derive(Clone, Copy)]
        struct Variables;

        engine
            .build_type::<ScriptResponse>()
            .register_indexer_get(
                |headers: &mut HeaderMap<HeaderValue>, key: ImmutableString| {
                    headers
                        .get(key.as_str())
                        .map(|v| v.to_str().unwrap().to_string())
                        .ok_or_else(|| {
                            Box::new(EvalAltResult::ErrorIndexNotFound(
                                key.into(),
                                Position::NONE,
                            ))
                        })
                },
            )
            .register_fn("parse_cookie", |s: ImmutableString| {
                Cookie::from_str(&s).map_err(|e| {
                    Box::new(EvalAltResult::ErrorSystem(
                        e.as_str().to_string(),
                        Box::new(e),
                    ))
                })
            })
            .register_fn(
                "with_expires",
                |s: ImmutableString, expires_at: Option<DateTime<Utc>>| PersistedVariable {
                    value: s.into(),
                    expires_at,
                },
            )
            .register_get("name", |cookie: &mut Cookie| cookie.name().to_string())
            .register_get("value", |cookie: &mut Cookie| cookie.value().to_string())
            .register_get("expires", |cookie: &mut Cookie| {
                cookie
                    .expires_datetime()
                    .map(|d| DateTime::from_timestamp(d.unix_timestamp(), 0).unwrap())
            })
            .register_set(
                "value",
                |persisted: &mut PersistedVariable, value: String| {
                    persisted.value = value;
                },
            )
            .register_set(
                "expires_at",
                |persisted: &mut PersistedVariable, expires_at: Option<DateTime<Utc>>| {
                    persisted.expires_at = expires_at;
                },
            )
            .on_print(|s| {
                for l in s.lines() {
                    println!("{} {}", owo_colors::OwoColorize::blue(&"[post-script]"), l);
                }
            })
            .on_debug(|s, src, pos| {
                debug_assert!(src.is_none());
                for l in s.lines() {
                    print!("{} ", owo_colors::OwoColorize::blue(&"[post-script]"));
                    if let (Some(line), Some(pos)) = (pos.line(), pos.position()) {
                        print!(
                            "{} ",
                            owo_colors::OwoColorize::cyan(&format!("[{}:{}]", line, pos))
                        );
                    }
                    println!("{}", l);
                }
            });

        for name in state.borrow().variables.keys() {
            let name = Rc::new(name.clone());
            engine.register_set(name.deref().clone(), {
                let name = name.clone();
                let config = config.clone();
                let state = state.clone();
                move |_v: &mut Variables, value: ImmutableString| {
                    state.borrow_mut().variables.insert(
                        name.deref().into(),
                        state::PersistedVariable {
                            value: value.into(),
                            expires_at: config
                                .get_variable(&name)
                                .unwrap()
                                .persist
                                .duration()
                                .map(|d| Utc::now() + d),
                        },
                    );
                }
            });

            engine.register_set(name.deref().clone(), {
                let state = state.clone();
                let name = name.clone();
                move |_v: &mut Variables, value: PersistedVariable| {
                    state
                        .borrow_mut()
                        .variables
                        .insert(name.deref().into(), value);
                }
            });

            engine.register_get(name.deref().clone(), {
                let state = state.clone();
                move |_v: &mut Variables| state.borrow_mut().variables[name.deref()].clone()
            });
        }

        let mut scope = Scope::new();

        scope.push("response", res);
        scope.push("vars", Variables);

        engine
            .run_ast_with_scope(&mut scope, &post_script)
            .map_err(|e| miette::miette!("{}", e))
            .context("Evaluating post-script")?;
    }

    Ok(())
}

fn run_variable(
    _cli: &Cli,
    var_cmd: &VariableCommand,
    _config: Config,
    state: Rc<RefCell<State>>,
) -> miette::Result<()> {
    match &var_cmd.command {
        cli::VariableSubCmd::Set {
            variable,
            value,
            expires,
        } => match (value, expires) {
            (Some(value), &expires) => {
                state.borrow_mut().variables.insert(
                    variable.clone(),
                    state::PersistedVariable {
                        value: value.clone(),
                        expires_at: expires.map(|e| Utc::now() + *e),
                    },
                );
            }
            (None, &Some(expires)) => {
                let mut state = state.borrow_mut();
                let Some(var) = state.variables.get_mut(&**variable) else {
                    bail!("Variable not set '{}'", variable);
                };

                var.expires_at = Some(Utc::now() + *expires);
            }
            (None, None) => {
                bail!("Variable value and/or expires must be set");
            }
        },
        cli::VariableSubCmd::Get { variable } => match state.borrow().variables.get(variable) {
            Some(v) => {
                {
                    use owo_colors::OwoColorize as _;
                    eprint!("{}   ", "Value:".blue());
                    let _ = std::io::stderr().flush(); // ensure the Value: is printed

                    println!("{}", v.value); // print to stdout so it can be piped

                    if let Some(expires_at) = v.expires_at {
                        eprintln!(
                            "{} {} {}",
                            "Expires:".blue(),
                            expires_at
                                .with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S"),
                            format!("({})", chrono_humanize::HumanTime::from(expires_at)).yellow(),
                        );
                    } else {
                        eprintln!("{} Never", "Expires:".blue());
                    }
                }
            }
            None => {
                use owo_colors::OwoColorize as _;
                eprintln!("{}", "Variable not defined".red());
            }
        },
        cli::VariableSubCmd::List => {
            // put into btreemap to have stable order
            let variables = &state.borrow().variables;
            let variables = BTreeMap::from_iter(variables);
            for (variable, v) in variables {
                {
                    use owo_colors::OwoColorize as _;

                    println!("{}", variable.green());

                    println!("  {}   {}", "Value:".blue(), v.value);

                    if let Some(expires_at) = v.expires_at {
                        println!(
                            "  {} {} {}",
                            "Expires:".blue(),
                            expires_at
                                .with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S"),
                            format!("({})", chrono_humanize::HumanTime::from(expires_at)).yellow(),
                        );
                    } else {
                        println!("  {} Never", "Expires:".blue());
                    }
                }
            }
        }
    }

    Ok(())
}

fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let config = Config::parse(config_str.parse()?)?;
    let state = Rc::new(RefCell::new(State::load(&cli.config)?));

    match &cli.subcmd {
        SubCmd::Query(s) => run_query(&cli, s, Rc::new(config), Rc::clone(&state))?,
        SubCmd::Variable(s) => run_variable(&cli, s, config, Rc::clone(&state))?,
    };

    state.borrow_mut().save(&cli.config)?;

    Ok(())
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let config_str = std::fs::read_to_string(&cli.config).into_diagnostic()?;
    run(cli, &config_str).map_err(|m| m.with_source_code(config_str))
}
