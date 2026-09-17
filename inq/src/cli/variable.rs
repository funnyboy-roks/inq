use std::{cell::RefCell, rc::Rc};

use crate::{
    cli::{Cli, VariableCommand},
    config::Config,
    state::State,
};

pub(crate) fn run(
    _cli: &Cli,
    var_cmd: &VariableCommand,
    _config: Config,
    state: &mut State,
) -> miette::Result<()> {
    // TODO(refactor)
    todo!();
    // match &var_cmd.command {
    //     VariableSubCmd::Set {
    //         variable,
    //         value,
    //         expires,
    //     } => match (value, expires) {
    //         (Some(value), &expires) => {
    //             state.borrow_mut().variables.insert(
    //                 variable.clone(),
    //                 PersistedVariable {
    //                     value: value.clone(),
    //                     expires_at: expires.map(|e| Utc::now() + *e),
    //                 },
    //             );
    //         }
    //         (None, &Some(expires)) => {
    //             let mut state = state.borrow_mut();
    //             let Some(var) = state.variables.get_mut(&**variable) else {
    //                 bail!("Variable not set '{}'", variable);
    //             };

    //             var.expires_at = Some(Utc::now() + *expires);
    //         }
    //         (None, None) => {
    //             bail!("Variable value and/or expires must be set");
    //         }
    //     },
    //     VariableSubCmd::Get { variable } => match state.borrow().variables.get(variable) {
    //         Some(v) => {
    //             print_variable(v, false);
    //         }
    //         None => {
    //             use owo_colors::OwoColorize as _;
    //             eprintln!("{}", "Variable not defined".red());
    //         }
    //     },
    //     VariableSubCmd::List => {
    //         // put into btreemap to have stable order
    //         let variables = &state.borrow().variables;
    //         let variables = BTreeMap::from_iter(variables);
    //         for (variable, v) in variables {
    //             println!("{}", owo_colors::OwoColorize::green(&variable));
    //             print_variable(v, true);
    //         }
    //     }
    // }

    Ok(())
}
