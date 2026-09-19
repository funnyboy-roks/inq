use std::path::Path;

use miette::NamedSource;

use crate::{
    cli::{Cli, RouteCommand, SubCmd},
    run,
};

pub fn make_cli(dir: &Path, route_name: &str) -> Cli {
    Cli {
        config: dir.join("main.inq"),
        subcmd: SubCmd::Route(RouteCommand {
            raw: false,
            client: Default::default(),
            route: Some(route_name.into()),
            args: Vec::new(),
        }),
    }
}

pub fn run_cli_test(tempdir: &Path, route: &str, config: String) -> miette::Result<()> {
    run(make_cli(tempdir, route), &config)
        .map_err(|m| m.with_source_code(NamedSource::new("inline config", config)))
}

#[macro_export]
macro_rules! eval {
    ($engine: expr, $($tt: tt)*) => {{
        let content = stringify!($($tt)*);
        let mut parser = inq_lang::Parser::new(&content)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap();
        let expr = parser
            .take_expr()
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
            .unwrap();

        $engine.global().eval(expr)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
    }};
}
