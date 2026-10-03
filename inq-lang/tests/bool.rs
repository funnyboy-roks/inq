use inq_lang::{assert_value, eval::Engine};

#[test]
fn literals() {
    let e = Engine::new();
    assert_value!(e, true => true);
    assert_value!(e, false => false);
}

#[test]
fn arithmetic() {
    let e = Engine::new();
    assert_value!(e, true  && true  => true );
    assert_value!(e, true  && false => false);
    assert_value!(e, false && true  => false);
    assert_value!(e, false && false => false);

    assert_value!(e, true  || true  => true );
    assert_value!(e, true  || false => true );
    assert_value!(e, false || true  => true );
    assert_value!(e, false || false => false);

    assert_value!(e, true  ^ true  => false);
    assert_value!(e, true  ^ false => true );
    assert_value!(e, false ^ true  => true );
    assert_value!(e, false ^ false => false);

    assert_value!(e, true  & true  => true );
    assert_value!(e, true  & false => false);
    assert_value!(e, false & true  => false);
    assert_value!(e, false & false => false);

    assert_value!(e, true  | true  => true );
    assert_value!(e, true  | false => true );
    assert_value!(e, false | true  => true );
    assert_value!(e, false | false => false);
}
