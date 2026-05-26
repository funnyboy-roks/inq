use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Instant};

use anstream::{eprintln, println};
use chrono::Utc;
use clap::Parser;
use miette::{Context, IntoDiagnostic, bail};
use reqwest::blocking::Client;

use crate::{
    cli::{Cli, QueryCommand, SubCmd, VariableCommand},
    config::Config,
    print::{print_request, print_response, print_variable},
    script::{ScriptResponse, script_engine},
    state::State,
};

mod cli;
mod config;
mod parse;
mod print;
mod script;
mod state;
mod util;

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

    print_request(&req, req_body)?;

    let start = Instant::now();
    let res = client.execute(req).into_diagnostic()?;
    let res = ScriptResponse::from_response(res, query_cmd.raw)?;
    let elapsed = start.elapsed();

    print_response(&res, elapsed)?;

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
                print_variable(v, false);
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
                println!("{}", owo_colors::OwoColorize::green(&variable));
                print_variable(v, true);
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
