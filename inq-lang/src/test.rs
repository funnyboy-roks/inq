use std::rc::Rc;

use miette::NamedSource;

use crate::{
    eval::{Engine, Scope, value::ValueRef},
    lex::Lexer,
    parse::Parser,
};

macro_rules! eval {
    ($engine: expr, $($tt: tt)*) => {{
        let content = stringify!($($tt)*);
        let lex = Lexer::new(&content);
        let mut parser = Parser::new(lex)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(NamedSource::new("literal", content.to_string()))
            })
            .unwrap();
        let expr = parser
            .take_expr()
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
            .unwrap();

        $engine.global().eval(expr)
            .map_err(|e| {
                miette::Report::from(e)
                    .with_source_code(NamedSource::new("literal", content.to_string()))
            })
            .unwrap()
    }};
}

#[test]
fn simple_expressions() {
    let e = Engine::new();
    assert_eq!(eval!(e, 1 + 2).downcast(), Some(3));
    assert_eq!(eval!(e, 1 + 2 * 3).downcast(), Some(7));
    assert_eq!(eval!(e, 1 + 2 * 3).downcast(), Some(7));
    assert_eq!(eval!(e, 1.0 + 2 * 3).downcast(), Some(7.0));
    assert_eq!(eval!(e, 5 / 10).downcast(), Some(0));
    assert_eq!(eval!(e, 5 / 10.).downcast(), Some(0.5));
    assert_eq!(eval!(e, 5. / 10).downcast(), Some(0.5));
    assert_eq!(eval!(e, 5. / 10.).downcast(), Some(0.5));
}

#[test]
fn conditions() {
    let e = Engine::new();
    assert_eq!(eval!(e, if 1 then 7 else 0).downcast(), Some(7));
    assert_eq!(eval!(e, if 0 then 7 else 0).downcast(), Some(0));
    assert_eq!(eval!(e, if true then 7 else 0).downcast(), Some(7));
    assert_eq!(eval!(e, if false then 7 else 0).downcast(), Some(0));
}

#[test]
fn cmp() {
    let e = Engine::new();
    // int <cmp> int
    assert_eq!(eval!(e, 1 < 0).downcast(), Some(false));
    assert_eq!(eval!(e, 1 <= 0).downcast(), Some(false));
    assert_eq!(eval!(e, 1 > 0).downcast(), Some(true));
    assert_eq!(eval!(e, 1 >= 0).downcast(), Some(true));
    assert_eq!(eval!(e, 1 == 0).downcast(), Some(false));
    assert_eq!(eval!(e, 1 != 0).downcast(), Some(true));

    // float <cmp> int
    assert_eq!(eval!(e, 1. < 0).downcast(), Some(false));
    assert_eq!(eval!(e, 1. <= 0).downcast(), Some(false));
    assert_eq!(eval!(e, 1. > 0).downcast(), Some(true));
    assert_eq!(eval!(e, 1. >= 0).downcast(), Some(true));
    assert_eq!(eval!(e, 1. == 0).downcast(), Some(false));
    assert_eq!(eval!(e, 1. != 0).downcast(), Some(true));

    // int <cmp> float
    assert_eq!(eval!(e, 1 < 0.).downcast(), Some(false));
    assert_eq!(eval!(e, 1 <= 0.).downcast(), Some(false));
    assert_eq!(eval!(e, 1 > 0.).downcast(), Some(true));
    assert_eq!(eval!(e, 1 >= 0.).downcast(), Some(true));
    assert_eq!(eval!(e, 1 == 0.).downcast(), Some(false));
    assert_eq!(eval!(e, 1 != 0.).downcast(), Some(true));

    // float <cmp> float
    assert_eq!(eval!(e, 1. < 0.).downcast(), Some(false));
    assert_eq!(eval!(e, 1. <= 0.).downcast(), Some(false));
    assert_eq!(eval!(e, 1. > 0.).downcast(), Some(true));
    assert_eq!(eval!(e, 1. >= 0.).downcast(), Some(true));
    assert_eq!(eval!(e, 1. == 0.).downcast(), Some(false));
    assert_eq!(eval!(e, 1. != 0.).downcast(), Some(true));
}
