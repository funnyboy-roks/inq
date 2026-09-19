use std::{rc::Rc, time::Duration};

use inq_lang::{
    IStr,
    eval::{
        registry::BinOp,
        value::{
            Value,
            native::{Float, Int},
        },
    },
};

#[derive(Debug, Clone, Copy)]
pub struct DurationValue(pub Duration);

impl From<humantime::Duration> for DurationValue {
    fn from(value: humantime::Duration) -> Self {
        Self(value.into())
    }
}

impl Value for DurationValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Duration".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", humantime::format_duration(self.0)).unwrap();
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(*self)
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", humantime::format_duration(self.0))
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_bin_op(BinOp::Add, |_, lhs, rhs: &Self| Self(lhs.0 + rhs.0));
        registry.register_bin_op(BinOp::Sub, |_, lhs, rhs: &Self| Self(lhs.0 - rhs.0));
        registry.register_method::<fn(&_) -> _>("secs", |this: &Self| this.0.as_secs_f64());
        registry.register_method::<fn(&_) -> _>("millis", |this: &Self| this.0.as_millis() as Int);
        registry.register_method::<fn(&_) -> _>("mins", |this: &Self| this.0.as_secs_f64() / 60.);
        registry.register_method::<fn(&_) -> _>("hours", |this: &Self| {
            this.0.as_secs_f64() / 60. / 60.
        });
        registry.register_method::<fn(&_) -> _>("days", |this: &Self| {
            this.0.as_secs_f64() / 60. / 60. / 24.
        });

        registry.register_static_method("parse", |ctx, seconds: IStr| {
            humantime::parse_duration(&seconds)
                .map(Self)
                .map_err(|e| ctx.error(format!("Unable to parse duration: {:?}", e)))
        });
        registry.register_static_method("from_millis", |ctx, millis: Int| {
            if millis < 0 {
                return Err(ctx.error("mills must be positive"));
            }
            Ok(Self(Duration::from_millis(millis as _)))
        });
        registry.register_static_method("from_secs", |ctx, seconds: Float| {
            if seconds < 0. {
                return Err(ctx.error("seconds must be positive"));
            }
            Ok(Self(Duration::from_secs_f64(seconds)))
        });
        registry.register_static_method("from_mins", |ctx, seconds: Float| {
            if seconds < 0. {
                return Err(ctx.error("seconds must be positive"));
            }
            Ok(Self(Duration::from_secs_f64(seconds * 60.)))
        });
        registry.register_static_method("from_hours", |ctx, hours: Float| {
            if hours < 0. {
                return Err(ctx.error("hours must be positive"));
            }
            Ok(Self(Duration::from_secs_f64(hours * 60. * 60.)))
        });
        registry.register_static_method("from_days", |ctx, days: Float| {
            if days < 0. {
                return Err(ctx.error("days must be positive"));
            }
            Ok(Self(Duration::from_secs_f64(days * 24. * 60. * 60.)))
        });
    }
}

#[cfg(test)]
mod test {
    use inq_lang::eval::value::native::Float;

    use crate::{
        eval,
        script::{base_engine, duration::DurationValue},
    };

    #[test]
    fn seconds() {
        let e = base_engine();

        let d = eval!(e, Duration.from_secs(69.42));
        let unwrapped = d.unwrap::<DurationValue>().0;
        assert_eq!(unwrapped.as_secs_f64(), 69.42);

        let d = eval!(e, Duration.from_secs(69.42).secs());
        let unwrapped = d.unwrap::<Float>();
        assert_eq!(unwrapped, 69.42);
    }

    #[test]
    fn minutes() {
        let e = base_engine();

        let d = eval!(e, Duration.from_mins(5.));
        let unwrapped = d.unwrap::<DurationValue>().0;
        assert_eq!(unwrapped.as_secs(), 300);

        let d = eval!(e, Duration.from_secs(345.).mins());
        let unwrapped = d.unwrap::<Float>();
        assert_eq!(unwrapped, 5.75);
    }

    #[test]
    fn parse() {
        let e = base_engine();

        let d = eval!(e, Duration.parse("5m"));
        let unwrapped = d.unwrap::<DurationValue>().0;
        assert_eq!(unwrapped.as_secs(), 300);

        let d = eval!(e, Duration.parse("5 days"));
        let unwrapped = d.unwrap::<DurationValue>().0;
        assert_eq!(unwrapped.as_secs(), 5 * 24 * 60 * 60);
    }
}
