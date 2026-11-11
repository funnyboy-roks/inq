use miette::IntoDiagnostic;
use syntect::parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};

use crate::{
    cli::{Cli, SubCmd},
    config::Config,
    state::State,
};

pub mod cli;
mod config;
// mod decode;
mod print;
mod script;
mod state;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod util;

pub fn load_syntax() -> SyntaxSet {
    let syntax = SyntaxDefinition::load_from_str(
        include_str!("../assets/inq.sublime-syntax"),
        true,
        Some("inq"),
    )
    .into_diagnostic()
    .expect("Valid syntax file");

    let mut builder = SyntaxSetBuilder::new();
    builder.add(syntax);
    builder.build()
}

pub fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let name = cli.config.file_name().unwrap().to_string_lossy();

    let config = Config::load(&name, config_str)?;
    let mut state = State::load(&cli.config)?;
    state.apply_variables(&config)?;

    match cli.subcmd {
        SubCmd::Route(cmd) => cli::route::run(cmd, config, &mut state)?,
        SubCmd::Variable(cmd) => cli::variable::run(cmd, config, &mut state)?,
        SubCmd::Eval(cmd) => cli::eval::run(cmd, config, &mut state)?,
    };

    state.save()?;

    Ok(())
}
