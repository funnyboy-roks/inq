use inq_lang::{
    assert_value,
    eval::{Engine, value::native::Int},
    eval_expr,
};

#[test]
fn it_exists() {
    let e = Engine::new();
    let x = eval_expr!(e, 34 + 35);

    let f = x.unwrap::<Int>();
    assert_eq!(f, 69);
}

#[test]
fn arithmetic() {
    let e = Engine::new();
    assert_value!(e, 15 + 25  => 40);
    assert_value!(e, 15 * 25  => 375);
    assert_value!(e, 15 - 25  => -10);
    assert_value!(e, 69 / 23  => 3);
    assert_value!(e, 69 / 20  => 3);
    assert_value!(e, -575     => -575);
    assert_value!(e, 15 || 25 => 15);
    assert_value!(e, 00 || 25 => 25);
}

/// not exhaustive, but should have some coverage
#[test]
fn float() {
    let e = Engine::new();
    assert_value!(e, 3 + 2.5        => 5.5);
    assert_value!(e, 3 * 2.5        => 7.5);
    assert_value!(e, 3 - 2.5        => 0.5);
    assert_value!(e, 3 - 5.5        => -2.5);
    assert_value!(e, 5 / 2.5        => 2.0);
    assert_value!(e, (2).to_float() => 2.0);
}

#[test]
fn cmp() {
    let e = Engine::new();
    assert_value!(e, 1 <  2 => true);
    assert_value!(e, 1 <= 2 => true);
    assert_value!(e, 1 >  2 => false);
    assert_value!(e, 1 >= 2 => false);
    assert_value!(e, 1 == 2 => false);
    assert_value!(e, 1 != 2 => true);

    assert_value!(e, 2 <  1 => false);
    assert_value!(e, 2 <= 1 => false);
    assert_value!(e, 2 >  1 => true);
    assert_value!(e, 2 >= 1 => true);
    assert_value!(e, 2 == 1 => false);
    assert_value!(e, 2 != 1 => true);

    assert_value!(e, 1 <  1 => false);
    assert_value!(e, 1 <= 1 => true);
    assert_value!(e, 1 >  1 => false);
    assert_value!(e, 1 >= 1 => true);
    assert_value!(e, 1 == 1 => true);
    assert_value!(e, 1 != 1 => false);
}

#[test]
fn parse() {
    let e = Engine::new();
    assert_value!(e, Int.parse("35")  => 35);
    assert_value!(e, Int.parse("-42") => -42);
    assert_value!(e, Int.parse("+49") => 49);

    eval_expr!(try e, Int.parse("this is not a valid int")).unwrap_err();
}
