use std::rc::Rc;

use crate::eval::{Engine, EvalResult, value::ValueRef};

pub fn eval_str(engine: Rc<Engine>, content: &str) -> EvalResult<ValueRef> {
    let mut parser = crate::Parser::new(content)
        .map_err(|e| {
            miette::Report::from(e).with_source_code(
                crate::source("literal", content.to_string()).with_language("inq"),
            )
        })
        .unwrap();

    let mut last = ValueRef::null();
    while let Some(e) = parser
        .take_expr()
        .map_err(|e| miette::Report::from(e).with_source_code(crate::source("literal", content)))
        .unwrap()
    {
        last = engine.global().eval(e)?;
    }

    Ok(last)
}

#[macro_export]
macro_rules! eval_expr {
    (try $engine: expr, $($tt:tt)*) => {{
        let content = stringify!($($tt)*);
        $crate::testing::eval_str(std::rc::Rc::clone(&$engine), content)
    }};
    ($engine: expr, $($tt:tt)*) => {{
        let content = stringify!($($tt)*);
        $crate::testing::eval_str(std::rc::Rc::clone(&$engine), content)
            .map_err(|e| {
                e.into_report("literal", content)
            })
            .unwrap()
    }};
}

#[macro_export]
macro_rules! assert_value {
    ($e:expr, raw($($inq:tt)*) => $val:expr) => {{
        let x = $crate::eval_expr!($e, $($inq)*);
        fn same_type<T>(_: T, _: T) {}
        let f = x.unwrap();
        same_type(&f, &$val);
        assert_eq!(f, $val);
    }};
    ($e:expr, raw($($inq:tt)*) as $ty:ty => $val:expr) => {{
        let x = $crate::eval_expr!($e, $($inq)*);
        let f = x.unwrap::<$ty>();
        assert_eq!(f, $val);
    }};
    ($e:expr, $inq:expr => (as $ty:ty) $val:expr) => {
        let x = $crate::eval_expr!($e, $inq);
        let f = x.unwrap::<$ty>();
        assert_eq!(f, $val);
    };
    ($e:expr, $inq:expr =>~ $val:expr) => {{
        let x = $crate::eval_expr!($e, $inq);
        assert_eq!(x, $val);
    }};
    ($e:expr, $inq:expr => $val:expr) => {{
        let x = $crate::eval_expr!($e, $inq);
        fn same_type<T>(_: T, _: T) {}
        let f = x.unwrap();
        same_type(&f, &$val);
        assert_eq!(f, $val);
    }};
}

#[macro_export]
macro_rules! assert_error {
    ($e:expr, $tt:tt => $err: ident) => {
        let x = $crate::eval_expr!(try $e, $tt);
        let f = x.unwrap_err();
        assert_matches!(f, $crate::eval::EvalError::$err { .. });
    };
    ($e:expr, raw($($tt:tt)*) => $err: ident) => {
        let x = $crate::eval_expr!(try $e, $($tt)*);
        let f = x.unwrap_err();
        assert_matches!(f, $crate::eval::EvalError::$err { .. });
    };
    ($e:expr, $inq:expr => $err: ident) => {
        let x = $crate::eval_expr!(try $e, $inq);
        let f = x.unwrap_err();
        assert_matches!(f, $crate::eval::EvalError::$err { .. });
    };
    ($e:expr, $inq:expr => $err:pat) => {
        let x = $crate::eval_expr!(try $e, $inq);
        let f = x.unwrap_err();
        assert_matches!(f, $err);
    };
}
