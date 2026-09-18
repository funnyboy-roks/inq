use std::{cell::RefCell, rc::Rc};

use inq_lang::{
    IStr,
    eval::{
        EvalError,
        value::{
            Value, ValueRef,
            native::{Array, Int, normalise_index},
        },
    },
};

#[derive(Debug, Clone)]
pub struct BytesValue {
    inner: Vec<u8>,
}

impl From<Vec<u8>> for BytesValue {
    fn from(value: Vec<u8>) -> Self {
        Self { inner: value }
    }
}

impl Value for BytesValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Bytes".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        let mut hex = hex::encode(&self.inner);
        if hex.len() > 20 {
            hex.truncate(20);
            hex.push('…');
        }
        write!(out, "Bytes(0x{})", hex).expect("Infallible");
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "Bytes(0x{})", hex::encode(&self.inner))
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&mut _) -> _>("len", |this| this.inner.len() as Int);
        registry.register_method::<fn(&mut _) -> _>("into_array", |this| {
            this.inner
                .iter()
                .copied()
                .map(Int::from)
                .map(Into::into)
                .collect::<Array>()
        });
        registry.register_method::<fn(&mut _) -> _>("into_string", |this| {
            Some(IStr::from(str::from_utf8(&this.inner).ok()?))
        });
        registry.register_index_get_set(
            |ctx, this, &idx: &Int| {
                Ok(Int::from(
                    this.inner[normalise_index(idx, this.inner.len(), ctx.index_span)?],
                ))
            },
            |ctx, this, &idx: &Int, value: ValueRef| {
                let idx = normalise_index(idx, this.inner.len(), ctx.index_span)?;
                let n = value.expect_downcast::<Int>(ctx.rhs_span)?;
                if !(0..=255).contains(&n) {
                    return Err(EvalError::custom(
                        ctx.rhs_span,
                        "Byte value must be in range [0, 255]",
                    ));
                }
                this.inner[idx] = n as _;
                Ok(())
            },
        );
    }
}
