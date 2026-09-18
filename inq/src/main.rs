use clap::Parser;

use inq::{cli::Cli, run};

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
