use clap::Parser;

use inq::{cli::Cli, run};
use miette::IntoDiagnostic;
use syntect::{
    highlighting::ThemeSet,
    parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder},
};

fn load_syntax() -> SyntaxSet {
    let syntax = SyntaxDefinition::load_from_str(
        include_str!("../../assets/inq.sublime-syntax"),
        true,
        Some("inq"),
    )
    .into_diagnostic()
    .expect("Valid syntax file");

    let mut builder = SyntaxSetBuilder::new();
    builder.add(syntax);
    builder.build()
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

    miette::set_hook(Box::new(|_| {
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-eighties.dark"].clone();
        Box::new(
            miette::MietteHandlerOpts::new()
                .with_syntax_highlighting(miette::highlighters::SyntectHighlighter::new(
                    load_syntax(),
                    theme,
                    false,
                ))
                .build(),
        )
    }))
    .unwrap();

    let name = cli
        .config
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    run(cli, &config_str).map_err(|m| m.with_source_code(inq_lang::source(name, config_str)))
}
