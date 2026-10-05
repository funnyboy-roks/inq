use crate::{eval::Engine, eval_expr};

#[test]
fn simple_expressions() {
    let e = Engine::new();
    assert_eq!(eval_expr!(e, 1 + 2).downcast(), Some(3));
    assert_eq!(eval_expr!(e, 1 + 2 * 3).downcast(), Some(7));
    assert_eq!(eval_expr!(e, 1 + 2 * 3).downcast(), Some(7));
    assert_eq!(eval_expr!(e, 1.0 + 2 * 3).downcast(), Some(7.0));
    assert_eq!(eval_expr!(e, 5 / 10).downcast(), Some(0));
    assert_eq!(eval_expr!(e, 5 / 10.).downcast(), Some(0.5));
    assert_eq!(eval_expr!(e, 5. / 10).downcast(), Some(0.5));
    assert_eq!(eval_expr!(e, 5. / 10.).downcast(), Some(0.5));
}

#[test]
fn conditions() {
    let e = Engine::new();
    assert_eq!(eval_expr!(e, if 1 then 7 else 0).downcast(), Some(7));
    assert_eq!(eval_expr!(e, if 0 then 7 else 0).downcast(), Some(0));
    assert_eq!(eval_expr!(e, if true then 7 else 0).downcast(), Some(7));
    assert_eq!(eval_expr!(e, if false then 7 else 0).downcast(), Some(0));
}

#[test]
fn cmp() {
    let e = Engine::new();
    // int <cmp> int
    assert_eq!(eval_expr!(e, 1 < 0).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1 <= 0).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1 > 0).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1 >= 0).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1 == 0).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1 != 0).downcast(), Some(true));

    // float <cmp> int
    assert_eq!(eval_expr!(e, 1. < 0).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1. <= 0).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1. > 0).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1. >= 0).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1. == 0).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1. != 0).downcast(), Some(true));

    // int <cmp> float
    assert_eq!(eval_expr!(e, 1 < 0.).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1 <= 0.).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1 > 0.).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1 >= 0.).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1 == 0.).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1 != 0.).downcast(), Some(true));

    // float <cmp> float
    assert_eq!(eval_expr!(e, 1. < 0.).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1. <= 0.).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1. > 0.).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1. >= 0.).downcast(), Some(true));
    assert_eq!(eval_expr!(e, 1. == 0.).downcast(), Some(false));
    assert_eq!(eval_expr!(e, 1. != 0.).downcast(), Some(true));
}
