use std::{fs, process::Command};

use tempfile::TempDir;

fn run_cli(config: &str, args: &[&str]) -> std::process::Output {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("api.inq");
    fs::write(&config_path, config).unwrap();

    Command::new(env!("CARGO_BIN_EXE_inq"))
        .arg("--config")
        .arg(config_path)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

fn list_routes(config: &str) -> String {
    let output = run_cli(config, &["route"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Route names and argument lists are styled even when stdout is captured.
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut plain = String::new();
    let mut chars = stdout.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.next() == Some('[') {
            for ch in chars.by_ref() {
                if ('@'..='~').contains(&ch) {
                    break;
                }
            }
        } else {
            plain.push(ch);
        }
    }
    plain
}

#[test]
fn listing_marks_each_default_argument_as_optional() {
    let output = list_routes(
        r#"
route mixed(required, flag = false, count = 0, text = "", nothing = null, computed = 1 + 2) => GET "/mixed";
route other(value = "default") => POST "/other";
"#,
    );

    let rows = output.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(rows.len(), 2, "{output}");
    assert!(
        rows[0].contains("mixed(required, flag?, count?, text?, nothing?, computed?)"),
        "{output}"
    );
    assert!(rows[0].ends_with("GET     \"/mixed\""), "{output}");
    assert!(rows[1].contains("other(value?)"), "{output}");
    assert!(rows[1].ends_with("POST    \"/other\""), "{output}");
}

#[test]
fn listing_does_not_evaluate_defaults_or_execute_routes() {
    let output = list_routes(
        r#"
route lazy(value = assert(false)) => GET "${assert(false)}"
    before { assert(false); }
    after { assert(false); }
"#,
    );

    assert!(output.contains("lazy(value?)"), "{output}");
    assert!(output.contains("${ assert(false) }"), "{output}");
}

#[test]
fn listing_preserves_required_arguments_and_routes_without_arguments() {
    let output = list_routes(
        r#"
route required(first, second) => GET "/required";
route health => GET "/health";
"#,
    );

    let rows = output.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(rows.len(), 2, "{output}");
    assert!(rows[0].contains("required(first, second)"), "{output}");
    assert!(
        rows[1]
            .split_whitespace()
            .eq(["health", "GET", "\"/health\""])
    );
    assert!(!output.contains('?'), "{output}");
}

#[test]
fn execution_uses_omitted_defaults_and_rejects_missing_required_arguments() {
    let mut server = mockito::Server::new();
    let config = format!(
        r#"
route mixed(required, flag = false, count = 0, text = "", nothing = null, computed = 1 + 2) => GET "{}/test/${{required}}/${{count}}/${{computed}}"
    before {{
        assert(!flag);
        print(text, nothing);
    }}
"#,
        server.url()
    );
    let mock = server.mock("GET", "/test/provided/0/3").create();

    let missing = run_cli(&config, &["route", "mixed"]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("Argument `required` is required"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let output = run_cli(&config, &["route", "mixed", "provided"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    mock.assert();
}
