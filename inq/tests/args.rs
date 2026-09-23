use clap::Parser;
use inq::{cli::Cli, testing::exec};
use tempfile::TempDir;

#[test]
fn default() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/default").create();

    let cli = Cli::parse_from(["inq", "r", "test"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn positional() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/positional").create();

    let cli = Cli::parse_from(["inq", "r", "test", "positional"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn named_long_space() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named").create();

    let cli = Cli::parse_from(["inq", "r", "test", "--arg", "arg=named"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn named_long_eq() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named").create();

    let cli = Cli::parse_from(["inq", "r", "test", "--arg=arg=named"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn named_short_space() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named").create();

    let cli = Cli::parse_from(["inq", "r", "test", "-a", "arg=named"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn named_short_eq() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named").create();

    let cli = Cli::parse_from(["inq", "r", "test", "-a=arg=named"]);
    exec(tempdir.path(), cli, config).unwrap();

    mock.assert();
}

#[test]
fn duplicated_named() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named1").create();

    let cli = Cli::parse_from(["inq", "r", "test", "-a=arg=named1", "-a=arg=named2"]);
    let err = exec(tempdir.path(), cli, config).unwrap_err();
    assert!(err.to_string().contains("specified twice")); // best assertion is by to_string here :/

    mock.expect(0);
}

#[test]
fn duplicated_pos_and_named() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg = "default") => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named1").create();

    let cli = Cli::parse_from([
        "inq",
        "r",
        "test",
        "positional",
        "-a=arg=named1",
        "-a=arg=named2",
    ]);
    let err = exec(tempdir.path(), cli, config).unwrap_err();
    assert!(err.to_string().contains("specified twice")); // best assertion is by to_string here :/

    mock.expect(0);
}

#[test]
fn missing_required() {
    let tempdir = TempDir::new().unwrap();
    let mut server = mockito::Server::new();
    let config = format!(r#"let BASE_URL = "{}";"#, server.url())
        + stringify! {
            route test(arg) => GET "/test/${arg}"
                before {
                    print(request.url);
                }
                after {
                    assert(response.status == 200);
                }
        };
    eprintln!("{}", config);

    let mock = server.mock("GET", "/test/named1").create();

    let cli = Cli::parse_from(["inq", "r", "test"]);
    let err = exec(tempdir.path(), cli, config).unwrap_err();
    assert!(err.to_string().contains("required")); // best assertion is by to_string here :/

    mock.expect(0);
}
