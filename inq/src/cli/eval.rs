use inq_lang::{Parser, eval::value::ValueRef};
use miette::{Context, IntoDiagnostic, NamedSource};

use crate::{cli::EvalCommand, config::Config, script, state::State};

pub fn run_inner(content: &str) -> miette::Result<()> {
    let mut parser = Parser::new(content)?;

    let engine = script::base_engine();

    let mut last = ValueRef::null();
    while let Some(e) = parser.take_expr()? {
        last = engine.global().eval(e)?;
    }

    eprintln!("{:?}", last.debug());

    Ok(())
}

pub fn run(cmd: EvalCommand, _config: Config, _state: &mut State) -> miette::Result<()> {
    let file = std::fs::read_to_string(cmd.file)
        .into_diagnostic()
        .context("Loading file")?;
    match run_inner(&file) {
        Ok(t) => Ok(t),
        Err(e) => {
            eprintln!(
                "{:?}",
                e.with_source_code(NamedSource::new("literal", file))
            );
            std::process::exit(1);
        }
    }
}
