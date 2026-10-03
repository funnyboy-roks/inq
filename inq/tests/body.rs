use inq::testing::run_cli_test;
use mockito::Matcher;
use tempfile::TempDir;

#[test]
fn text() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => POST "/test"
                before {
                    request.body = "hello";
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("POST", "/test")
        .match_body("hello")
        .match_header("content-type", "text/plain; charset=utf-8")
        .with_status(200)
        .create();
    run_cli_test(tempdir.path(), "test", config).unwrap();

    mock.assert();
}

#[test]
fn json() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => POST "/test"
                before {
                    request.body = json({ key: "value" });
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("POST", "/test")
        .match_body(&*serde_json::json!({ "key": "value" }).to_string())
        .match_header("content-type", "application/json")
        .with_status(200)
        .create();
    run_cli_test(tempdir.path(), "test", config).unwrap();

    mock.assert();
}

#[test]
fn empty() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => POST "/test"
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("POST", "/test")
        .match_body(Matcher::Missing)
        .with_status(200)
        .create();
    run_cli_test(tempdir.path(), "test", config).unwrap();

    mock.assert();
}
