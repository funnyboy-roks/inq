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
            named_args: Vec::new(),
        }),
    }
}

pub fn run_cli_test(tempdir: &Path, route: &str, config: impl Into<String>) -> miette::Result<()> {
    let config = config.into();
    run(make_cli(tempdir, route), &config)
        .map_err(|m| m.with_source_code(NamedSource::new("inline config", config)))
}

pub fn exec(tempdir: &Path, mut cli: Cli, config: impl Into<String>) -> miette::Result<()> {
    let config = config.into();
    cli.config = tempdir.join("main.inq");
    run(cli, &config).map_err(|m| m.with_source_code(NamedSource::new("inline config", config)))
}
