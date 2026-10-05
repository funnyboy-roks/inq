use std::{cell::RefCell, rc::Rc};

use clap::Parser;

use crate::{
    cli::{Cli, SubCmd},
    config::Config,
    state::State,
};

mod cli;
mod config;
mod decode;
mod parse;
mod print;
mod script;
mod state;
mod util;

fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let config = Config::parse(config_str.parse()?)?;
    let state = Rc::new(RefCell::new(State::load(&cli.config)?));

    match &cli.subcmd {
        SubCmd::Query(s) => cli::query::run(&cli, s, Rc::new(config), Rc::clone(&state))?,
        SubCmd::Variable(s) => cli::variable::run(&cli, s, config, Rc::clone(&state))?,
    };

    state.borrow_mut().save(&cli.config)?;

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
