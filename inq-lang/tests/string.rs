use inq_lang::{
    IStr, StringExt, assert_value,
    eval::{Engine, value::native::Array},
    eval_expr,
};

#[test]
fn it_exists() {
    let e = Engine::new();
    let x = eval_expr!(e, "hello");

    let f = x.unwrap::<IStr>();
    assert_eq!(f, "hello");
}

#[test]
fn concat() {
    let e = Engine::new();
    assert_value!(e, "hello " + "world" => IStr::from("hello world"));
}

#[test]
fn methods() {
    let e = Engine::new();
    assert_value!(e, "hello".len() => 5);
    assert_eq!(
        eval_expr!(e, "a,b,c".split(",")).unwrap::<Array>(),
        ["a".intern(), "b".intern(), "c".intern()]
    );
    assert_value!(e, "this is a string".replace(" ", "-") => "this-is-a-string".intern());
    assert_value!(e, "tacocat".substring(1, -1) => "acoca".intern());
    assert_eq!(
        eval_expr!(e, "hello".chars()).unwrap::<Array>(),
        &['h', 'e', 'l', 'l', 'o']
            .into_iter()
            .map(IStr::from)
            .collect::<Vec<_>>()[..]
    );
    assert_value!(e, "    hello    ".trim() => "hello".intern());
}
