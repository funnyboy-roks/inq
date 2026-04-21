use std::{borrow::Cow, fmt::Display, io::Write, time::Instant};

use anstream::{eprintln, println};
use clap::Parser;
use miette::{Context, IntoDiagnostic};
use reqwest::{blocking::Client, header};
use serde_json::Value as JsonValue;

use crate::{cli::Cli, config::Config};

mod cli;
mod config;
mod util;

fn pretty_print_json(w: &mut impl Write, json: JsonValue, indent: usize) -> std::io::Result<()> {
    use owo_colors::OwoColorize as _;

    fn apply_indent(w: &mut impl Write, indent: usize) -> std::io::Result<()> {
        write!(w, "\n{0:>1$}", "", indent * 4)
    }

    match json {
        JsonValue::Null => write!(w, "{}", "null".bright_black()),
        JsonValue::Bool(b) => write!(w, "{}", b.cyan()),
        JsonValue::Number(number) => write!(w, "{}", number.yellow()),
        JsonValue::String(_) => write!(w, "{}", json.green()),
        JsonValue::Array(values) => {
            write!(w, "[")?;
            if !values.is_empty() {
                for (i, v) in values.into_iter().enumerate() {
                    if i > 0 {
                        write!(w, ",")?;
                    }
                    apply_indent(w, indent + 1)?;
                    pretty_print_json(w, v, indent + 1)?;
                }
                apply_indent(w, indent)?;
            }
            write!(w, "]")
        }
        JsonValue::Object(map) => {
            write!(w, "{{")?;
            if !map.is_empty() {
                for (i, (k, v)) in map.into_iter().enumerate() {
                    if i > 0 {
                        write!(w, ",")?;
                    }
                    apply_indent(w, indent + 1)?;
                    write!(w, "{}: ", JsonValue::String(k).blue().bold())?;
                    pretty_print_json(w, v, indent + 1)?;
                }
                apply_indent(w, indent)?;
            }
            write!(w, "}}")
        }
    }
}

fn run(cli: Cli, config_str: &str) -> miette::Result<()> {
    let config = Config::parse(config_str.parse()?)?;

    let client = Client::new();

    let query = config.get_query(&cli.query)?.context("Query not defined")?;

    let (req, req_body) = query.to_request(&client, |n| {
        if let Some(v) = cli.get_variable(n) {
            Ok(Some(String::from(v)))
        } else {
            Ok(config.get_variable(n)?.map(Cow::into_owned))
        }
    })?;

    {
        use owo_colors::OwoColorize as _;

        eprintln!("{}:", "Request Details".cyan());

        eprintln!(
            "  {}:   {:?}",
            "HTTP Version".blue(),
            req.version().yellow()
        );
        eprintln!("  {}:         {}", "Method".blue(), req.method().yellow());
        eprintln!("  {}:            {}", "URL".blue(), req.url().yellow());
        if let Some(timeout) = req.timeout() {
            eprintln!("  {}:            {:?}", "Timeout".blue(), timeout.yellow());
        }

        eprintln!("  {}:", "Headers".blue());
        for (name, value) in req.headers() {
            match value.to_str() {
                Ok(s) => eprintln!("    {}: {}", name.yellow(), s),
                Err(_) => eprintln!("    {}: {:?}", name.yellow(), value),
            }
        }

        if let Some(req_body) = req_body {
            match req_body {
                config::PopulatedBody::Json(value) => {
                    eprintln!("{}:", "Request Body (JSON)".cyan());
                    pretty_print_json(&mut anstream::stderr().lock(), value, 0)
                        .into_diagnostic()?;
                    eprintln!();
                }
                config::PopulatedBody::Text(cow) => {
                    eprintln!("{}:", "Response Body (Raw)".cyan());

                    eprintln!("{}", cow);
                }
            }
        }

        eprintln!("{}", "Sending Request...".purple());
    }

    let start = Instant::now();
    let mut res = client.execute(req).into_diagnostic()?;
    let elapsed = start.elapsed();

    {
        use owo_colors::OwoColorize as _;

        eprintln!("{}:", "Response Details".cyan());
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
        eprintln!(
            "  {}:   {:?}",
            "HTTP Version".blue(),
            res.version().yellow()
        );
        eprintln!("  {}:            {}", "URL".blue(), res.url().yellow());
        eprintln!("  {}:       {:?}", "Duration".blue(), elapsed.yellow());
        eprintln!("  {}:    {}", "Status Code".blue(), status);
        if let Some(remote_addr) = res.remote_addr() {
            eprintln!("  {}: {}", "Remote Address".blue(), remote_addr.yellow());
        }

        eprintln!("  {}:", "Headers".blue());
        for (name, value) in res.headers() {
            match value.to_str() {
                Ok(s) => eprintln!("    {}: {}", name.yellow(), s),
                Err(_) => eprintln!("    {}: {:?}", name.yellow(), value),
            }
        }

        if !cli.raw
            && let Some(header) = res.headers().get(header::CONTENT_TYPE)
            && header == "application/json"
        {
            eprintln!("{}:", "Response Body (JSON)".cyan());
            let json: JsonValue = res.json().into_diagnostic()?;
            pretty_print_json(&mut anstream::stdout().lock(), json, 0).into_diagnostic()?;
            println!();
        } else {
            eprintln!("{}:", "Response Body (Raw)".cyan());

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
