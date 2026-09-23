use inq_lang::{
    assert_value, eval,
    eval::{
        Engine,
        value::native::{Float, Int},
    },
};

#[test]
fn it_exists() {
    let e = Engine::new();
    let x = eval!(e, 1.5 + 2.5);

    let f = x.unwrap::<Float>();
    assert_eq!(f, 1.5 + 2.5);
}

#[test]
fn arithmetic() {
    let e = Engine::new();
    assert_value!(e, 1.5 + 2.5  => 4.0);
    assert_value!(e, 1.5 * 2.5  => 3.75);
    assert_value!(e, 1.5 - 2.5  => -1.0);
    assert_value!(e, 1.5 / 2.5  => 0.6);
    assert_value!(e, -5.75      => -5.75);
    assert_value!(e, 1.5 || 2.5 => 1.5);
    assert_value!(e, 0.0 || 2.5 => 2.5);
}

#[test]
fn statik() {
    let e = Engine::new();
    assert_value!(e, Float.inf     => f64::INFINITY);
    assert_value!(e, Float.neg_inf => f64::NEG_INFINITY);
    // because nan != nan
    assert!(eval!(e, Float.nan).unwrap::<Float>().is_nan());
}

/// not exhaustive, but should have some coverage
#[test]
fn methods() {
    let e = Engine::new();
    assert_value!(e, (9.0).sqrt()   => 3.0);
    assert_value!(e, (9.5).floor()  => 9.0);
    assert_value!(e, (9.5).ceil()   => 10.0);
    assert_value!(e, (9.75).round() => 10.0);
}

/// not exhaustive, but should have some coverage
#[test]
fn int() {
    let e = Engine::new();
    assert_value!(e, 1.5 + 2      => 3.5);
    assert_value!(e, 1.5 * 2      => 3.0);
    assert_value!(e, 1.5 - 2      => -0.5);
    assert_value!(e, 1.5 / 2      => 0.75);
    assert_value!(e, 2.0.to_int() => (as Int) 2);
}

#[test]
fn cmp() {
    let e = Engine::new();
    assert_value!(e, 1.5 <  2.5 => true);
    assert_value!(e, 1.5 <= 2.5 => true);
    assert_value!(e, 1.5 >  2.5 => false);
    assert_value!(e, 1.5 >= 2.5 => false);
    assert_value!(e, 1.5 == 2.5 => false);
    assert_value!(e, 1.5 != 2.5 => true);

    assert_value!(e, 2.5 <  1.5 => false);
    assert_value!(e, 2.5 <= 1.5 => false);
    assert_value!(e, 2.5 >  1.5 => true);
    assert_value!(e, 2.5 >= 1.5 => true);
    assert_value!(e, 2.5 == 1.5 => false);
    assert_value!(e, 2.5 != 1.5 => true);

    assert_value!(e, 1.5 <  1.5 => false);
    assert_value!(e, 1.5 <= 1.5 => true);
    assert_value!(e, 1.5 >  1.5 => false);
    assert_value!(e, 1.5 >= 1.5 => true);
    assert_value!(e, 1.5 == 1.5 => true);
    assert_value!(e, 1.5 != 1.5 => false);
}

#[test]
fn parse() {
    let e = Engine::new();
    assert_value!(e, Float.parse("3.5")    => 3.5);
    assert_value!(e, Float.parse("3.1e3")  => 3100.0);
    assert_value!(e, Float.parse("2.5e-2") => 0.025);
    assert_value!(e, Float.parse("inf")    => Float::INFINITY);
    assert_value!(e, Float.parse("-inf")   => Float::NEG_INFINITY);
    assert_value!(e, Float.parse("7")      => 7.0);
    assert_value!(e, Float.parse("007")    => 7.0);
    assert_value!(e, Float.parse(".5")     => 0.5);
    assert_value!(e, Float.parse("0.5")    => 0.5);

    eval!(try e, Float.parse("this is not a valid float")).unwrap_err();
}
