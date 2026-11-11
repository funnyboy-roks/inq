use std::collections::BTreeMap;

use chrono::Utc;
use miette::bail;

use crate::{
    cli::{VariableCommand, VariableSubCmd},
    config::Config,
    print::print_variable,
    state::State,
};

pub fn run(var_cmd: VariableCommand, _config: Config, state: &mut State) -> miette::Result<()> {
    match var_cmd.command {
        VariableSubCmd::Set {
            variable,
            value,
            expires,
        } => match (value, expires) {
            (Some(value), Some(expires)) => {
                state.set_persisted_var(variable, value, Some(Utc::now() + *expires));
            }
            (None, Some(expires)) => {
                let Some(var) = state.get_persisted_var(&variable) else {
                    bail!("Variable not set '{}'", variable);
                };

                state.set_persisted_var(variable, var.value, Some(Utc::now() + *expires));
            }
            (Some(value), None) => {
                let existing = state.get_persisted_var(&variable);

                state.set_persisted_var(variable, value, existing.and_then(|e| e.expires_at));
            }
            (None, None) => {
                bail!("Variable value and/or expires must be set");
            }
        },
        VariableSubCmd::Get { variable } => match state.get_persisted_var(variable) {
            Some(v) => {
                print_variable(v, false);
            }
            None => {
                use owo_colors::OwoColorize as _;
                eprintln!("{}", "Variable not defined".red());
                std::process::exit(1);
            }
        },
        VariableSubCmd::List => {
            // put into btreemap to have stable order
            let variables = BTreeMap::from_iter(state.persisted_variables());
            for (variable, v) in variables {
                println!("{}", owo_colors::OwoColorize::green(&variable));
                print_variable(v, true);
            }
        }
    }
    Ok(())
}
