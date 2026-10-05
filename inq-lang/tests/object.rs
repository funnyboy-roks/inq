use std::assert_matches;

use inq_lang::{
    StringExt, assert_error, assert_value,
    eval::{
        Engine,
        value::{
            ValueRef,
            native::{Array, Null, Object},
        },
    },
    eval_expr,
};

#[test]
fn literals() {
    let e = Engine::new();
    assert_eq!(eval_expr!(e, {}).unwrap::<Object>().0.borrow().len(), 0);
    assert_eq!(
        eval_expr!(e, { key: "value" }).unwrap::<Object>(),
        Object::from_iter([("key".intern(), "value".intern())])
    );
    assert_eq!(
        eval_expr!(e, { "key": "value" }).unwrap::<Object>(),
        Object::from_iter([("key".intern(), "value".intern())])
    );
}

#[test]
fn methods() {
    let e = Engine::new();
    assert_value!(e, raw( { "key0": "value", "key1": "value" }.len() ) => 2);
    assert_value!(e,
        raw( { "key0": "value", "key1": "value" }.keys() ) as Array
            => ["key0".intern(), "key1".intern()]
    );
    assert_value!(e,
        raw({ "key0": "value", "key1": 42 }.values()) as Array
            => [ValueRef::new("value".intern()), 42.into()]
    );
}

#[test]
fn index() {
    let e = Engine::new();
    assert_value!(e, raw( { "key": "value" }["key"]                    ) => "value".intern());
    assert_value!(e, raw( { "key": "value", "key1": "value2" }["key1"] ) => "value2".intern());
}

#[test]
fn index_oob() {
    let e = Engine::new();
    assert_error!(e, raw( {}["test"]                 )  => MissingIndex);
    assert_error!(e, raw( { "key": "value" }["key2"] ) => MissingIndex);
}

#[test]
fn try_index_oob() {
    let e = Engine::new();
    assert_value!(e, raw( {}["test"]?                 ) => Null);
    assert_value!(e, raw( { "key": "value" }["key2"]? ) => Null);
}

#[test]
fn index_wrong_type() {
    let e = Engine::new();
    assert_error!(e, raw( { "key": "value" }[69]  ) => InvalidIndex);
    assert_error!(e, raw( { "key": "value" }[[]]  ) => InvalidIndex);
    assert_error!(e, raw( { "key": "value" }[4.5] ) => InvalidIndex);
    assert_error!(e, raw( { "key": "value" }[{}]  ) => InvalidIndex);
}

#[test]
fn field() {
    let e = Engine::new();
    assert_value!(e, raw( { "key": "value" }.key  ) => "value".intern());
}

#[test]
fn field_invalid() {
    let e = Engine::new();
    assert_error!(e, raw( { "key": "value" }.key0 ) => UnknownField);
}

#[test]
fn field_method_conflict() {
    let e = Engine::new();
    assert_value!(e, raw( { "len": "some value" }.len   ) => "some value".intern());
    assert_value!(e, raw( { "len": "some value" }.len() ) => 1);
}
