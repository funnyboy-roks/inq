use std::{cell::RefCell, rc::Rc};

use inq_lang::{
    IStr, StringExt,
    eval::{
        EvalError, EvalResult,
        registry::Registry,
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Float, Int, Null, Object},
        },
    },
};

#[derive(Debug, Clone)]
pub(crate) struct Json(pub serde_json::Value);
impl Value for Json {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Json".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self.0).unwrap();
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }

    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&mut _) -> _>("to_object", |this| Self::to_value(&this.0));
    }
}

impl Json {
    pub fn from_value(ctx: CallContext, arg: ValueRef) -> EvalResult<Self> {
        Json::json_inner(&ctx, arg).map(Self)
    }

    pub fn json_inner(ctx: &CallContext, arg: ValueRef) -> EvalResult<serde_json::Value> {
        #[allow(clippy::redundant_pattern_matching)]
        if let Some(s) = arg.borrow().downcast_ref::<IStr>() {
            Ok(serde_json::Value::String(s.into()))
        } else if let Some(i) = arg.downcast::<Int>() {
            Ok(serde_json::Value::from(i))
        } else if let Some(f) = arg.downcast::<Float>() {
            Ok(serde_json::Value::from(f))
        } else if let Some(b) = arg.downcast::<bool>() {
            Ok(serde_json::Value::from(b))
        } else if let Some(_) = arg.downcast::<Null>() {
            Ok(serde_json::Value::from(()))
        } else if let Some(a) = arg.downcast::<Array>() {
            a.into_iter().map(|a| Self::json_inner(ctx, a)).collect()
        } else if let Some(o) = arg.downcast::<Object>() {
            o.0.into_iter()
                .map(|(k, v)| {
                    let x = Self::json_inner(ctx, v)?;
                    Ok((k.as_str().to_string(), x))
                })
                .collect()
        } else {
            Err(ctx.error(format!("Invalid type for JSON: {}", arg.type_name_of())))
        }
    }

    fn to_value(v: &serde_json::Value) -> ValueRef {
        match v {
            serde_json::Value::Null => ValueRef::null(),
            &serde_json::Value::Bool(b) => b.into(),
            serde_json::Value::Number(n) => {
                if let Some(n) = n.as_i64() {
                    n.into()
                } else {
                    n.as_f64()
                        .expect("Numbers outside of i64 are not supported")
                        .into()
                }
            }
            serde_json::Value::String(s) => s.intern().into(),
            serde_json::Value::Array(a) => a.iter().map(Self::to_value).collect::<Array>().into(),
            serde_json::Value::Object(o) => o
                .iter()
                .map(|(k, v)| (&**k, Self::to_value(v)))
                .collect::<Object>()
                .into(),
        }
    }
}

#[cfg(test)]
mod test {
    use inq_lang::{
        IStr, Parser,
        eval::value::native::{Float, Int, Null, Object},
    };
    use miette::NamedSource;

    use super::Json;

    use crate::script::base_engine;

    macro_rules! eval {
        ($engine: expr, $($tt: tt)*) => {{
            let content = stringify!($($tt)*);
            let mut parser = Parser::new(&content)
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap();
            let expr = parser
                .take_expr()
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap()
                .unwrap();

            $engine.global().eval(expr)
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap()
        }};
    }

    #[test]
    fn roundtrip() {
        let e = base_engine();
        let j = eval!(e, {
            _before: { "key1": "bar", "key2": 0, "key3": 0.5, "key4": true, "key5": false, "key6": null },
            _after: json({ "key1": "bar", "key2": 0, "key3": 0.5, "key4": true, "key5": false, "key6": null }).to_object()
        });

        let obj = j.unwrap::<Object>().0;
        let before = obj["_before"].unwrap::<Object>().0;
        let after = obj["_after"].unwrap::<Object>().0;

        macro_rules! assert_key {
            ($key:literal as $ty:ty) => {
                assert_eq!(before[$key].unwrap::<$ty>(), after[$key].unwrap::<$ty>());
            };
        }

        assert_key!("key1" as IStr);
        assert_key!("key2" as Int);
        assert_key!("key3" as Float);
        assert_key!("key4" as bool);
        assert_key!("key5" as bool);
        assert_key!("key6" as Null);

        dbg!(obj);
    }

    #[test]
    fn valid_json() {
        let e = base_engine();
        let j = eval!(e, json({
            "key1": "bar",
            "key2": 0,
            "key3": 0.5,
            "key4": true,
            "key5": false,
            "key6": null,
            "key7": {},
            "key8": { "key1": 0 },
            "key9": { "key1": { "key1": { "key1": 0 } } },
            "key10": [],
            "key11": [0],
            "key12": [{ "key1": { "key1": { "key1": 0 } } }],
            "key13": [[0]],
            "key14": [0, ["hello"], { "foo": "bar" }],
        }));

        assert_eq!(
            j.unwrap::<Json>().0,
            serde_json::json!({
                "key1": "bar",
                "key2": 0,
                "key3": 0.5,
                "key4": true,
                "key5": false,
                "key6": null,
                "key7": {},
                "key8": { "key1": 0 },
                "key9": { "key1": { "key1": { "key1": 0 } } },
                "key10": [],
                "key11": [0],
                "key12": [{ "key1": { "key1": { "key1": 0 } } }],
                "key13": [[0]],
                "key14": [0, ["hello"], { "foo": "bar" }],
            })
        )
    }

    #[test]
    fn valid_json_unquoted() {
        let e = base_engine();
        let j = eval!(e, json({
                key1: "bar",
                key2: 0,
                key3: 0.5,
                key4: true,
                key5: false,
                key6: null,
                key7: {},
                key8: { key1: 0 },
                key9: { key1: { key1: { key1: 0 } } },
                key10: [],
                key11: [0],
                key12: [{ key1: { key1: { key1: 0 } } }],
                key13: [[0]],
                key14: [0, ["hello"], { foo: "bar" }],
        }));

        assert_eq!(
            j.unwrap::<Json>().0,
            serde_json::json!({
                "key1": "bar",
                "key2": 0,
                "key3": 0.5,
                "key4": true,
                "key5": false,
                "key6": null,
                "key7": {},
                "key8": { "key1": 0 },
                "key9": { "key1": { "key1": { "key1": 0 } } },
                "key10": [],
                "key11": [0],
                "key12": [{ "key1": { "key1": { "key1": 0 } } }],
                "key13": [[0]],
                "key14": [0, ["hello"], { "foo": "bar" }],
            })
        )
    }
}
