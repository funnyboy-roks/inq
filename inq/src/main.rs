use clap::Parser;

use inq::{cli::Cli, run};
use miette::NamedSource;

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let config_str = match std::fs::read_to_string(&cli.config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Unable to read '{}': {}", cli.config.display(), e);
            std::process::exit(1);
        }
    };
    let name = cli
        .config
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    run(cli, &config_str).map_err(|m| m.with_source_code(NamedSource::new(name, config_str)))
}
