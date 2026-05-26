use std::{cell::RefCell, collections::BTreeMap, fmt::Display, io::Write, rc::Rc, time::Instant};

use anstream::{eprintln, println};
use chrono::Utc;
use clap::Parser;
use miette::{Context, IntoDiagnostic, bail};
use reqwest::blocking::Client;
use serde_json::Value as JsonValue;

use crate::{
    cli::{Cli, QueryCommand, SubCmd, VariableCommand},
    config::Config,
    script::{ScriptBody, ScriptResponse, script_engine},
    state::State,
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

    let vars = Rc::new(config.load_variables(&cli.subcmd, &state.borrow())?);

    for (name, val) in &*vars {
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
        for (name, value) in &*res.headers {
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

        let (engine, mut scope) = script_engine(state, config, vars);
        scope.push("response", res);

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
