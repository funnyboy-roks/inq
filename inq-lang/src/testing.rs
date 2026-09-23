#[macro_export]
macro_rules! eval {
    (try $engine: expr, $($tt: tt)*) => {{
        let content = stringify!($($tt)*);
        let mut parser = inq_lang::Parser::new(&content)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap();
        let expr = parser
            .take_expr()
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
            .unwrap();

        $engine.global().eval(expr)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
    }};
    ($engine: expr, $($tt: tt)*) => {{
        let content = stringify!($($tt)*);
        let mut parser = inq_lang::Parser::new(&content)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap();
        let expr = parser
            .take_expr()
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
            .unwrap();

        $engine.global().eval(expr)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(miette::NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
    }};
}

#[macro_export]
macro_rules! assert_value {
    ($e:expr, $inq:expr => (as $ty:ty) $val:expr) => {
        let x = $crate::eval!($e, $inq);
        let f = x.unwrap::<$ty>();
        assert_eq!(f, $val);
    };
    ($e:expr, $inq:expr =>~ $val:expr) => {{
        let x = $crate::eval!($e, $inq);
        assert_eq!(x, $val);
    }};
    ($e:expr, $inq:expr => $val:expr) => {{
        let x = $crate::eval!($e, $inq);
        fn same_type<T>(_: T, _: T) {}
        let f = x.unwrap();
        same_type(&f, &$val);
        assert_eq!(f, $val);
    }};
}
