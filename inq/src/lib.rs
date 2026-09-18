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

    match &cli.subcmd {
        SubCmd::Route(s) => cli::route::run(&cli, s, config, &mut state)?,
        SubCmd::Variable(s) => cli::variable::run(&cli, s, config, &mut state)?,
    };

    state.save(&cli.config)?;

    Ok(())
}
