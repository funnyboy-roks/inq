use std::{borrow::Cow, fmt::Display, time::Instant};

use anstream::{ColorChoice, eprintln, print, println};
use clap::Parser;
use miette::{Context, IntoDiagnostic};
use reqwest::header;
use serde_json::Value as JsonValue;

use crate::{cli::Cli, config::Config};

mod cli;
mod config;
mod util;

fn pretty_print_json(json: JsonValue, indent: usize) {
    use owo_colors::OwoColorize as _;

    fn apply_indent(indent: usize) {
        print!("\n{0:>1$}", "", indent * 4);
    }

    match json {
        JsonValue::Null => print!("{}", "null".bright_black()),
        JsonValue::Bool(b) => print!("{}", b.blue()),
        JsonValue::Number(number) => print!("{}", number.yellow()),
        JsonValue::String(_) => print!("{}", json.green()),
        JsonValue::Array(values) => {
            print!("[");
            if !values.is_empty() {
                for (i, v) in values.into_iter().enumerate() {
                    if i > 0 {
                        print!(",");
                    }
                    apply_indent(indent + 1);
                    pretty_print_json(v, indent + 1);
                }
                apply_indent(indent);
            }
            print!("]");
        }
        JsonValue::Object(map) => {
            print!("{{");
            if !map.is_empty() {
                for (i, (k, v)) in map.into_iter().enumerate() {
                    if i > 0 {
                        print!(",");
                    }
                    apply_indent(indent + 1);
                    print!("{}: ", JsonValue::String(k).blue().bold());
                    pretty_print_json(v, indent + 1);
                }
                apply_indent(indent);
            }
            print!("}}");
        }
    }
}

fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let config = Config::parse(config_str.parse()?)?;

    let query = config.get_query(&cli.query)?.context("Query not defined")?;

    let req = query.to_request(|n| {
        if let Some(v) = cli.get_variable(n) {
            Ok(Some(String::from(v)))
        } else {
            Ok(config.get_variable(n)?.map(Cow::into_owned))
        }
    })?;

    let start = Instant::now();
    let mut res = req.send().into_diagnostic()?;
    let elapsed = start.elapsed();

    {
        use owo_colors::OwoColorize as _;

        let status = res.status();
        let status: &dyn Display = if status.is_server_error() {
            &status.bright_red()
        } else if status.is_client_error() {
            &status.red()
        } else if status.is_informational() {
            &status.blue()
        } else if status.is_redirection() {
            &status.yellow()
        } else if status.is_success() {
            &status.green()
        } else {
            &status
        };
        eprintln!("{}:   {:?}", "HTTP Version".blue(), res.version().yellow());
        eprintln!("{}:       {:?}", "Duration".blue(), elapsed.yellow());
        eprintln!("{}:    {}", "Status Code".blue(), status);
        if let Some(remote_addr) = res.remote_addr() {
            eprintln!("{}: {}", "Remote Address".blue(), remote_addr.yellow());
        }

        eprintln!("{}:", "Headers".blue());
        for (name, value) in res.headers() {
            match value.to_str() {
                Ok(s) => eprintln!("  {}: {}", name.yellow(), s),
                Err(_) => eprintln!("  {}: {:?}", name.yellow(), value),
            }
        }

        if !cli.raw
            && let Some(header) = res.headers().get(header::CONTENT_TYPE)
            && header == "application/json"
        {
            eprintln!("{}:", "Body (JSON)".blue());
            let json: JsonValue = res.json().into_diagnostic()?;
            pretty_print_json(json, 0);
            println!();
        } else {
            eprintln!("{}:", "Body (Raw)".blue());

            std::io::copy(&mut res, &mut std::io::stdout().lock()).into_diagnostic()?;
        }
    }

    Ok(())
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let config_str = std::fs::read_to_string(&cli.config).into_diagnostic()?;
    run(cli, &config_str).map_err(|m| m.with_source_code(config_str))
}
