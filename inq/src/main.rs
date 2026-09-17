use clap::Parser;

use crate::{
    cli::{Cli, SubCmd},
    config::Config,
    state::State,
};

mod cli;
mod config;
mod decode;
mod print;
mod script;
mod state;
mod util;

fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let config = Config::load(config_str)?;
    let mut state = State::load(&cli.config)?;

    match &cli.subcmd {
        SubCmd::Route(s) => cli::route::run(&cli, s, config, &mut state)?,
        SubCmd::Variable(s) => cli::variable::run(&cli, s, config, &mut state)?,
    };

    state.save(&cli.config)?;

    Ok(())
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let config_str = match std::fs::read_to_string(&cli.config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Unable to read '{}': {}", cli.config.display(), e);
            std::process::exit(1);
        }
    };
    run(cli, &config_str).map_err(|m| m.with_source_code(config_str))
}
