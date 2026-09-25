use inq::{run, testing::make_cli};
use tempfile::TempDir;

macro_rules! method {
    (fn $name:ident => $method:ident) => {
        #[test]
        fn $name() {
            let tempdir = TempDir::new().unwrap();
            let mut server = mockito::Server::new();
            let config = format!(r#"let BASE_URL = "{}";"#, server.url())
                + stringify! {
                    route test => $method "/test";
                };
            eprintln!("{}", config);
            let mock = server
                .mock(stringify!($method), "/test")
                .with_status(200)
                .create();
            run(make_cli(tempdir.path(), "test"), &config).unwrap();

            mock.assert();
        }
    };
}

method!(fn get => GET);
method!(fn head => HEAD);
method!(fn post => POST);
method!(fn put => PUT);
method!(fn delete => DELETE);
method!(fn options => OPTIONS);
method!(fn trace => TRACE);
method!(fn patch => PATCH);
