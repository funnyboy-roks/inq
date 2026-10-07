use std::{fs, process::Command};

use tempfile::TempDir;

fn list_routes(config: &str) -> String {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("api.inq");
    fs::write(&config_path, config).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_inq"))
        .args(["--config", config_path.to_str().unwrap(), "route"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn strip_ansi(output: &str) -> String {
    let mut plain = String::with_capacity(output.len());
    let mut chars = output.chars();
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

fn assert_method_column_aligns(output: &str) {
    let plain = strip_ansi(output);
    let lines = plain.lines().collect::<Vec<_>>();
    let header_column = lines[0].find("Method").unwrap();
    for line in &lines[1..] {
        assert_eq!(
            line.find("GET"),
            Some(header_column),
            "route row is misaligned: {line:?}\n{output}"
        );
    }
}

#[test]
fn route_methods_align_when_argument_lists_differ() {
    let output = list_routes(
        r#"
route foo(a, b, c) => GET "/foo";
route bar(a) => GET "/bar";
route baz => GET "/baz";
"#,
    );

    assert_method_column_aligns(&output);
}

#[test]
fn route_table_omits_the_args_column_when_no_route_has_arguments() {
    let output = list_routes(r#"route health => GET "/health";"#);
    let header = output.lines().next().unwrap();

    assert!(!header.contains("(Args)"), "{output}");
    assert_method_column_aligns(&output);
}
