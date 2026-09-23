use std::{cell::RefCell, rc::Rc};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use inq_lang::{
    IStr,
    eval::{
        EvalError,
        value::{
            Value, ValueRef,
            native::{Array, Int, normalise_index, normalise_index_error},
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesValue {
    inner: RefCell<Vec<u8>>,
}

impl From<Vec<u8>> for BytesValue {
    fn from(value: Vec<u8>) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
}

impl From<BytesValue> for Vec<u8> {
    fn from(value: BytesValue) -> Self {
        value.inner.into_inner()
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
        let mut hex = hex::encode(&*self.inner.borrow());
        if hex.len() > 20 {
            hex.truncate(20);
            hex.push('…');
        }
        write!(out, "Bytes(0x{})", hex).expect("Infallible");
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn eq(&self, other: ValueRef) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "Bytes(0x{})", hex::encode(&*self.inner.borrow()))
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_static_method("from_hex", |ctx, hex: IStr| {
            hex::decode(&*hex)
                .map(BytesValue::from)
                .map_err(|e| ctx.error(format!("Unable to decode hex: {}", e)))
        });
        registry.register_static_method("from_base64", |ctx, base64: IStr| {
            BASE64_STANDARD
                .decode(&*base64)
                .map(BytesValue::from)
                .map_err(|e| ctx.error(format!("Unable to decode hex: {}", e)))
        });
        registry.register_static_method("from_array", |ctx, array: Array| {
            let array = array.into_vec();
            let mut out = Vec::with_capacity(array.len());
            for v in array {
                if let Some(n) = v.downcast::<Int>()
                    && (0..=255).contains(&n)
                {
                    out.push(n as u8);
                } else {
                    return Err(ctx.error(format!(
                        "All values in array must be integers within [0, 255], got {}",
                        v.type_name_of()
                    )));
                }
            }
            Ok(Self::from(out))
        });

        registry.register_method::<fn(&_) -> _>("len", |this| this.inner.borrow().len() as Int);
        registry.register_method::<fn(&_) -> _>("to_array", |this| {
            this.inner
                .borrow()
                .iter()
                .copied()
                .map(Int::from)
                .collect::<Array>()
        });
        registry.register_method::<fn(&_) -> _>("to_utf8_string", |this| {
            Some(IStr::from(str::from_utf8(&this.inner.borrow()).ok()?))
        });
        registry.register_method::<fn(&_) -> _>("to_hex", |this| {
            Some(IStr::from(hex::encode(&*this.inner.borrow())))
        });
        registry.register_method::<fn(&_) -> _>("to_base64", |this| {
            Some(IStr::from(BASE64_STANDARD.encode(&*this.inner.borrow())))
        });
        registry.register_index_get_set(
            |_, this, &idx: &Int| {
                let this = this.inner.borrow();
                Ok(normalise_index(idx, this.len())?
                    .map(|n| this[n])
                    .map(Int::from))
            },
            |ctx, this, &idx: &Int, value: ValueRef| {
                let mut this = this.inner.borrow_mut();
                let idx = normalise_index_error(idx, this.len(), ctx.index_span)?;
                let n = value.expect_downcast::<Int>(ctx.rhs_span)?;
                if !(0..=255).contains(&n) {
                    return Err(EvalError::custom(
                        ctx.rhs_span,
                        "Byte value must be in range [0, 255]",
                    ));
                }
                this[idx] = n as _;
                Ok(())
            },
        );
    }
}

#[cfg(test)]
mod test {
    use base64::Engine;
    use inq_lang::{IStr, eval_expr};

    use crate::script::{base_engine, bytes_value::BytesValue};

    #[test]
    fn from_hex() {
        let e = base_engine();
        let bytes = eval_expr!(
            e,
            Bytes.from_hex("746869732069732068657861646563696d616c21")
        );

        assert_eq!(
            bytes.unwrap::<BytesValue>().inner.into_inner(),
            hex::decode("746869732069732068657861646563696d616c21").unwrap()
        );
    }

    #[test]
    fn to_hex() {
        let e = base_engine();
        let bytes = eval_expr!(e, Bytes.from_array([250, 202, 222]).to_hex());

        assert_eq!(bytes.unwrap::<IStr>(), "facade");
    }

    #[test]
    fn from_base64() {
        let e = base_engine();
        let bytes = eval_expr!(e, Bytes.from_base64("dGhpcyBpcyBiYXNlNjQh"));

        assert_eq!(
            bytes.unwrap::<BytesValue>().inner.into_inner(),
            base64::engine::general_purpose::STANDARD
                .decode("dGhpcyBpcyBiYXNlNjQh")
                .unwrap()
        );
    }

    #[test]
    fn to_base64() {
        let e = base_engine();
        let bytes = eval_expr!(e, Bytes.from_array([250, 202, 222]).to_base64());

        assert_eq!(bytes.unwrap::<IStr>(), "+sre");
    }
}
