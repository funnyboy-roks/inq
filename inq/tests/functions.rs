use clap::Parser;
use inq::{cli::Cli, testing::exec};
use inq_lang::{assert_err_is_variant, eval::EvalError};
use tempfile::TempDir;

#[test]
fn request() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            let calls = 0;

            fn print_url() with request {
                print(request.url);
                calls += 1;
            }

            route test() => GET "/test"
                before {
                    print_url();
                    assert(calls == 1);
                }
                after {
                    assert_success();
                }
        };
    eprintln!("{}", config);

    let mock = server
        .mock("GET", "/test")
        .with_body("hello world")
        .create();

    let cli = Cli::parse_from(["inq", "r", "test"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn request_invalid() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            let calls = 0;

            fn print_url() with request {
                print(request.url);
                calls += 1;
            }

            route test() => GET "/test"
                after {
                    print_url(); // not allowed in after
                    assert_success();
                }
        };
    eprintln!("{}", config);

    let mock = server
        .mock("GET", "/test")
        .with_body("hello world")
        .create();

    let cli = Cli::parse_from(["inq", "r", "test"]);
    let err = exec(tempdir.path(), cli, config).unwrap_err();

    assert_err_is_variant!(err, EvalError::RequestInBadPosition);

    mock.expect(0);
}

#[test]
fn response() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            let calls = 0;

            fn print_body() with response {
                print(response.text());
                calls += 1;
            }

            route test() => GET "/test"
                after {
                    assert_success();
                    print_body();
                    assert(calls == 1);
                }
        };
    eprintln!("{}", config);

    let mock = server
        .mock("GET", "/test")
        .with_body("hello world")
        .create();

    let cli = Cli::parse_from(["inq", "r", "test"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn response_invalid() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            let calls = 0;

            fn print_body() with response {
                print(response.text());
                calls += 1;
            }

            route test() => GET "/test"
                before {
                    print_body(); // not allowed in before
                }
                after {
                    assert_success();
                }
        };
    eprintln!("{}", config);

    let mock = server
        .mock("GET", "/test")
        .with_body("hello world")
        .create();

    let cli = Cli::parse_from(["inq", "r", "test"]);
    let err = exec(tempdir.path(), cli, config).unwrap_err();

    assert_err_is_variant!(err, EvalError::ResponseInBadPosition);

    mock.expect(0);
}
