use std::path::Path;

use crate::cli::{Cli, RouteCommand, SubCmd};

pub fn make_cli(dir: &Path, route_name: &str) -> Cli {
    Cli {
        config: dir.join("main.inq"),
        subcmd: SubCmd::Route(RouteCommand {
            raw: false,
            route: Some(route_name.into()),
            args: Vec::new(),
        }),
    }
}
