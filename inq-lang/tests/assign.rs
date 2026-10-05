use inq_lang::{
    eval::{Engine, value::native::Int},
    eval_expr,
};

#[test]
fn assign_int() {
    let e = Engine::new();
    macro_rules! op {
        ($init:literal $t:tt $x:literal => $v:literal) => {
            assert_eq!(
                eval_expr! { e,
                    let x = $init;
                    x $t $x;
                    x
                }
                .unwrap::<Int>(),
                $v,
            );
        };
    }
    op!(5 += 3 => 8);
    op!(5 -= 3 => 2);
    op!(5 *= 3 => 15);
    op!(15 /= 3 => 5);
    op!(5 %= 3 => 2);
    op!(5 |= 3 => 7);
    op!(5 &= 3 => 1);
    op!(5 ^= 3 => 6);
}
