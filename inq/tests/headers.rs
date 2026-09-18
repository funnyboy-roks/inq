use inq::{run, testing::make_cli};
use mockito::Matcher;
use tempfile::TempDir;

#[test]
fn req_unset() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => GET "/test";
        };
    eprintln!("{}", config);
    let mock = server
        .mock("GET", "/test")
        .match_request(|req| {
            req.headers().len() == 3
                && req.has_header("user-agent")
                && req.has_header("accept")
                && req.has_header("host")
        })
        .match_header("user-agent", Matcher::Regex("inq/.*".into()))
        .with_status(200)
        .create();
    run(make_cli(tempdir.path(), "test"), &config).unwrap();

    mock.assert();
}

#[test]
fn req_content_type() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => GET "/test"
                before {
                    request.headers["content-type"] = "image/png";
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("GET", "/test")
        .match_header("content-type", "image/png")
        .with_status(200)
        .create();
    run(make_cli(tempdir.path(), "test"), &config).unwrap();

    mock.assert();
}

#[test]
fn req_arbitrary() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => GET "/test"
                before {
                    request.headers["my-header-123"] = "my header value";
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("GET", "/test")
        .match_header("my-header-123", "my header value")
        .with_status(200)
        .create();
    run(make_cli(tempdir.path(), "test"), &config).unwrap();

    mock.assert();
}

#[test]
fn res_content_type() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => GET "/test"
                after {
                    debug(response.headers);
                    assert(response.headers["content-type"] == "image/png");
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("GET", "/test")
        .with_header("content-type", "image/png")
        .with_status(200)
        .create();
    run(make_cli(tempdir.path(), "test"), &config).unwrap();

    mock.assert();
}

#[test]
fn res_arbitrary() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test => GET "/test"
                after {
                    debug(response.headers);
                    assert(response.headers["my-header-123"] == "my header value");
                }
        };
    eprintln!("{}", config);
    let mock = server
        .mock("GET", "/test")
        .with_header("my-header-123", "my header value")
        .with_status(200)
        .create();
    run(make_cli(tempdir.path(), "test"), &config).unwrap();

    mock.assert();
}
