use std::{
    assert_matches,
    sync::atomic::{AtomicUsize, Ordering},
};

use inq_lang::{
    eval::{
        Engine, EvalError,
        value::native::{Int, Null},
    },
    eval_expr,
};

#[test]
fn call_works() {
    let e = Engine::new();

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    e.global().declare_function("inc", |_ctx, ()| {
        COUNT.fetch_add(1, Ordering::SeqCst);
    });

    let v = eval_expr! { e,
        fn foo() {
            inc();
        }
        foo();
        foo();
        foo();
        foo();
    };

    assert!(v.is::<Null>());
    assert_eq!(COUNT.load(Ordering::SeqCst), 4);
}

#[test]
fn implicit_return_value() {
    let e = Engine::new();

    let v = eval_expr! { e,
        fn foo() {
            5
        }
        foo()
    };

    assert_eq!(v, 5);
}

#[test]
fn explicit_return_value() {
    let e = Engine::new();

    e.global()
        .declare_function("unreachable", |_ctx, ()| unreachable!() as ());

    let v = eval_expr! { e,
        fn foo() {
            return 5;
            unreachable();
        }
        foo()
    };

    assert_eq!(v, 5);
}

#[test]
fn arrow() {
    let e = Engine::new();

    let v = eval_expr! { e,
        fn foo() => 5;
        foo()
    };

    assert_eq!(v, 5);
}

#[test]
fn arg() {
    let e = Engine::new();

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    e.global().declare_function("inc", |_ctx, n: Int| {
        COUNT.fetch_add(n as usize, Ordering::SeqCst);
    });

    let v = eval_expr! { e,
        fn foo(n) {
            inc(n);
        }
        foo(5);
    };

    assert!(v.is::<Null>());
    assert_eq!(COUNT.load(Ordering::SeqCst), 5);
}

#[test]
fn arg_implicit_return() {
    let e = Engine::new();

    let v = eval_expr! { e,
        fn foo(n) {
            n + 2
        }
        foo(5)
    };

    assert_eq!(v, 7);
}

#[test]
fn arg_explicit_return() {
    let e = Engine::new();

    e.global()
        .declare_function("unreachable", |_ctx, ()| unreachable!() as ());

    let v = eval_expr! { e,
        fn foo(n) {
            return n + 2;
            unreachable();
        }
        foo(5)
    };

    assert_eq!(v, 7);
}

#[test]
fn arg_arrow_return() {
    let e = Engine::new();

    let v = eval_expr! { e,
        fn foo(n) => n + 2;
        foo(5)
    };

    assert_eq!(v, 7);
}

#[test]
fn correct_return() {
    let e = Engine::new();

    let v = eval_expr! { e,
        fn foo() {
            fn bar() {
                return 5;
            }
            return bar() + 2;
        }
        foo()
    };

    assert_eq!(v, 7);
}

#[test]
fn correct_return_arrow() {
    let e = Engine::new();

    let v = eval_expr! { e,
        fn foo() {
            fn bar() => return 6;
            return bar() + 2;
        }
        foo()
    };

    assert_eq!(v, 8);
}

#[test]
fn recursion() {
    let e = Engine::new();

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    e.global().declare_function("inc", |_ctx, ()| {
        COUNT.fetch_add(1, Ordering::SeqCst);
    });

    let v = eval_expr! { e,
        fn foo(n) {
            inc();
            if n > 0 {
                foo(n - 1);
            }
        }
        foo(4);
    };

    assert!(v.is::<Null>());
    assert_eq!(COUNT.load(Ordering::SeqCst), 5);
}

#[test]
fn recursion_unbounded() {
    let e = Engine::new();

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    e.global().declare_function("inc", |_ctx, ()| {
        COUNT.fetch_add(1, Ordering::SeqCst);
    });

    let v = eval_expr! { try e,
        fn foo() {
            foo();
        }
        foo();
    };

    assert_matches!(v.unwrap_err(), EvalError::RecursionDepth { .. });
}
