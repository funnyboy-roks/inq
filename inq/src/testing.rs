use std::path::Path;

use crate::cli::{Cli, RouteCommand, SubCmd};

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
