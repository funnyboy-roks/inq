use std::assert_matches;

use inq_lang::{
    IStr, StringExt, assert_error, assert_value,
    eval::{
        Engine,
        value::native::{Array, Null},
    },
    eval_expr,
};

#[test]
fn literals() {
    let e = Engine::new();
    assert_eq!(eval_expr!(e, []).unwrap::<Array>(), [] as [IStr; 0]);
    assert_eq!(eval_expr!(e, [1]).unwrap::<Array>(), [1]);
    assert_eq!(eval_expr!(e, [1, 2, 3]).unwrap::<Array>(), [1, 2, 3]);
}

#[test]
fn methods() {
    let e = Engine::new();
    assert_value!(e, [].len() => 0);
    assert_value!(e, [1].len() => 1);
    assert_value!(e, [1, 2, 3].len() => 3);
    assert_eq!(
        eval_expr! { e,
            let a = [1, 2, 3];
            a.push(4);
            a
        }
        .unwrap::<Array>(),
        [1, 2, 3, 4],
    );
    assert_eq!(
        eval_expr! { e,
            let a = [1, 2, 3];
            a.push(4, 5, 6);
            a
        }
        .unwrap::<Array>(),
        [1, 2, 3, 4, 5, 6],
    );
}

#[test]
fn index() {
    let e = Engine::new();
    assert_value!(e, ["a"][0]            => "a".intern());
    assert_value!(e, ["a"][-1]           => "a".intern());
    assert_value!(e, ["a", "b", "c"][0]  => "a".intern());
    assert_value!(e, ["a", "b", "c"][-1] => "c".intern());
    assert_value!(e, ["a", "b", "c"][1]  => "b".intern());
    assert_value!(e, ["a", "b", "c"][-2] => "b".intern());
    assert_value!(e, ["a", "b", "c"][2]  => "c".intern());
    assert_value!(e, ["a", "b", "c"][-3] => "a".intern());
}

#[test]
fn index_oob() {
    let e = Engine::new();
    assert_error!(e, ["a"][1]            => MissingIndex);
    assert_error!(e, ["a"][-2]           => MissingIndex);
    assert_error!(e, ["a", "b", "c"][3]  => MissingIndex);
    assert_error!(e, ["a", "b", "c"][-4] => MissingIndex);
}

#[test]
fn try_index_oob() {
    let e = Engine::new();
    assert_value!(e, ["a"][1]?            => Null);
    assert_value!(e, ["a"][-2]?           => Null);
    assert_value!(e, ["a", "b", "c"][3]?  => Null);
    assert_value!(e, ["a", "b", "c"][-4]? => Null);
}

#[test]
fn index_wrong_type() {
    let e = Engine::new();
    assert_error!(e, ["a"]["string"]      => InvalidIndex);
    assert_error!(e, ["a"][[]]            => InvalidIndex);
    assert_error!(e, ["a", "b", "c"][4.5] => InvalidIndex);
    assert_error!(e, ["a", "b", "c"][{}]  => InvalidIndex);
}
