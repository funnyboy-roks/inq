use std::{fmt::Display, io::Write, time::Duration};

use miette::{Context, IntoDiagnostic};
use reqwest::blocking::Request;
use serde_json::Value as JsonValue;

use crate::{
    config::{self, PopulatedBody},
    script::{ContentType, ScriptResponse},
    state::PersistedVariable,
    util::DATETIME_FORMAT,
};

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

pub fn print_request(req: &Request, body: Option<PopulatedBody<'_>>) -> miette::Result<()> {
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

    if let Some(req_body) = body {
        match req_body {
            config::PopulatedBody::Json(value) => {
                eprintln!("{}:", "Request Body (JSON)".cyan());
                pretty_print_json(&mut anstream::stderr().lock(), value, 0).into_diagnostic()?;
                eprintln!();
            }
            config::PopulatedBody::Text(cow) => {
                eprintln!("{}:", "Request Body (Raw)".cyan());

                eprintln!("{}", cow);
            }
        }
    }

    eprintln!("{}", "Sending Request...".purple());

    Ok(())
}

pub fn print_response(res: &ScriptResponse, elapsed: Duration, raw: bool) -> miette::Result<()> {
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
    eprintln!("  {}:   {:?}", "HTTP Version".blue(), res.version.yellow());
    eprintln!("  {}:            {}", "URL".blue(), res.url.yellow());
    eprintln!("  {}:       {:?}", "Duration".blue(), elapsed.yellow());
    eprintln!("  {}:    {}", "Status Code".blue(), status);
    if let Some(remote_addr) = res.remote_addr {
        eprintln!("  {}: {}", "Remote Address".blue(), remote_addr.yellow());
    }

    eprintln!("  {}:", "Headers".blue());
    for (name, value) in &*res.headers {
        match value.to_str() {
            Ok(s) => eprintln!("    {}: {}", name.yellow(), s),
            Err(_) => eprintln!("    {}: {:?}", name.yellow(), value),
        }
    }

    if !res.body.is_empty() {
        let encoding = match res.body.encoding() {
            Some(e) => format!(" ({})", e),
            None => "".into(),
        };
        let encoding = encoding.cyan();

        match res.body.content_type() {
            Some(ContentType::Json) if !raw => {
                eprintln!("{}{}:", "Response Body (JSON)".cyan(), encoding);
                let json = res.body.json().context("Parsing response body as json")?;
                pretty_print_json(&mut anstream::stdout().lock(), json.clone(), 0)
                    .into_diagnostic()?;
                println!();
            }
            Some(ContentType::Text) if !raw => {
                eprintln!("{}{}:", "Response Body (Plaintext)".cyan(), encoding);
                let text = res
                    .body
                    .text()
                    .context("Parsing response body as plaintext")?;
                println!("{}", text);
            }
            None | Some(_) => {
                eprintln!("{}{}:", "Response Body (Raw)".cyan(), encoding);
                std::io::stdout()
                    .write_all(&res.body.bytes()?)
                    .into_diagnostic()
                    .context("Writing response body")?;
            }
        }
    }

    Ok(())
}

pub fn print_variable(v: &PersistedVariable, indent: bool) {
    use owo_colors::OwoColorize as _;
    if indent {
        eprint!("  ");
    }
    eprint!("{}   ", "Value:".blue());
    let _ = std::io::stderr().flush(); // ensure the Value: is printed

    println!("{}", v.value); // print to stdout so it can be piped

    if indent {
        eprint!("  ");
    }
    if let Some(expires_at) = v.expires_at {
        eprintln!(
            "{} {} {}",
            "Expires:".blue(),
            expires_at
                .with_timezone(&chrono::Local)
                .format(DATETIME_FORMAT),
            format!("({})", chrono_humanize::HumanTime::from(expires_at)).yellow(),
        );
    } else {
        eprintln!("{} Never", "Expires:".blue());
    }
}

#[macro_export]
macro_rules! warn {
    ($($tt: tt)*) => {{
        use owo_colors::OwoColorize as _;

        eprint!("{}: ", "WARNING".yellow().bold());
        eprintln!($($tt)*);
    }};
}
