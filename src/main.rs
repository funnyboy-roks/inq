use clap::Parser;
use miette::{Context, IntoDiagnostic};

use crate::{cli::Cli, config::Config};

mod cli;
mod config;
mod util;

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let config = std::fs::read_to_string(&cli.config).into_diagnostic()?;
    let config = Config::parse(config.parse()?)?;

    let query = config.get_query(&cli.query)?.context("Query not defined")?;

    let req = query.to_request(|n| {
        cli.var
            .iter()
            .find(|v| v.name == n)
            .map(|v| &*v.value)
            .or_else(|| config.get_variable(n))
    })?;

    let res = req.send().into_diagnostic()?;
    let txt = res.text().into_diagnostic()?;
    println!("{}", txt);

    Ok(())
}
