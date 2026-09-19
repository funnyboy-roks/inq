use std::{cell::RefCell, rc::Rc};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use inq_lang::{
    IStr,
    eval::{
        EvalError,
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Int, normalise_index},
        },
    },
};

#[derive(Debug, Clone, derive_more::From)]
pub struct BytesValue {
    inner: Vec<u8>,
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
        registry.register_static_method::<fn(_, _) -> _>(
            "from_hex",
            |ctx: CallContext, hex: IStr| {
                hex::decode(&*hex)
                    .map(BytesValue::from)
                    .map_err(|e| ctx.error(format!("Unable to decode hex: {}", e)))
            },
        );
        registry.register_static_method::<fn(_, _) -> _>(
            "from_base64",
            |ctx: CallContext, base64: IStr| {
                BASE64_STANDARD
                    .decode(&*base64)
                    .map(BytesValue::from)
                    .map_err(|e| ctx.error(format!("Unable to decode hex: {}", e)))
            },
        );
        registry.register_static_method::<fn(_, _) -> _>(
            "from_array",
            |ctx: CallContext, array: Array| {
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
            },
        );

        registry.register_method::<fn(&mut _) -> _>("len", |this| this.inner.len() as Int);
        registry.register_method::<fn(&mut _) -> _>("to_array", |this| {
            this.inner
                .iter()
                .copied()
                .map(Int::from)
                .map(Into::into)
                .collect::<Array>()
        });
        registry.register_method::<fn(&mut _) -> _>("to_utf8_string", |this| {
            Some(IStr::from(str::from_utf8(&this.inner).ok()?))
        });
        registry.register_method::<fn(&mut _) -> _>("to_hex", |this| {
            Some(IStr::from(hex::encode(&this.inner)))
        });
        registry.register_method::<fn(&mut _) -> _>("to_base64", |this| {
            Some(IStr::from(BASE64_STANDARD.encode(&this.inner)))
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

#[cfg(test)]
mod test {
    use base64::Engine;
    use inq_lang::IStr;

    use crate::{
        eval,
        script::{base_engine, bytes_value::BytesValue},
    };

    #[test]
    fn from_hex() {
        let e = base_engine();
        let bytes = eval!(
            e,
            Bytes.from_hex("746869732069732068657861646563696d616c21")
        );

        assert_eq!(
            bytes.unwrap::<BytesValue>().inner,
            hex::decode("746869732069732068657861646563696d616c21").unwrap()
        );
    }

    #[test]
    fn to_hex() {
        let e = base_engine();
        let bytes = eval!(e, Bytes.from_array([250, 202, 222]).to_hex());

        assert_eq!(bytes.unwrap::<IStr>(), "facade");
    }

    #[test]
    fn from_base64() {
        let e = base_engine();
        let bytes = eval!(e, Bytes.from_base64("dGhpcyBpcyBiYXNlNjQh"));

        assert_eq!(
            bytes.unwrap::<BytesValue>().inner,
            base64::engine::general_purpose::STANDARD
                .decode("dGhpcyBpcyBiYXNlNjQh")
                .unwrap()
        );
    }

    #[test]
    fn to_base64() {
        let e = base_engine();
        let bytes = eval!(e, Bytes.from_array([250, 202, 222]).to_base64());

        assert_eq!(bytes.unwrap::<IStr>(), "+sre");
    }
}
