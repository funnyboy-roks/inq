use crate::{
    cli::{Cli, SubCmd},
    config::Config,
    state::State,
};

pub mod cli;
mod config;
mod decode;
mod print;
mod script;
mod state;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod util;

pub fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let config = Config::load(config_str)?;
    let mut state = State::load(&cli.config)?;

    match cli.subcmd {
        SubCmd::Route(cmd) => cli::route::run(cmd, config, &mut state)?,
        SubCmd::Variable(cmd) => cli::variable::run(cmd, config, &mut state)?,
        SubCmd::Eval(cmd) => cli::eval::run(cmd, config, &mut state)?,
    };

    state.save()?;

    Ok(())
}
