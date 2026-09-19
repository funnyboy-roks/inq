use std::{cell::RefCell, rc::Rc};

use chrono::{DateTime, Utc};
use chrono_humanize::Humanize;
use cookie::time::OffsetDateTime;
use inq_lang::{
    IStr, StringExt,
    eval::{
        registry::BinOp,
        value::{CallContext, Value},
    },
};

use crate::script::duration::DurationValue;

#[derive(Debug, Clone, derive_more::From)]
pub struct DateTimeValue {
    inner: DateTime<Utc>,
}

impl From<OffsetDateTime> for DateTimeValue {
    fn from(value: OffsetDateTime) -> Self {
        DateTimeValue {
            inner: DateTime::from_timestamp_nanos(value.unix_timestamp_nanos().try_into().unwrap()),
        }
    }
}

impl Value for DateTimeValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "DateTime".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self.inner).unwrap();
    }

    fn snapshot(&self) -> std::rc::Rc<std::cell::RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "DateTime({})", self.inner.humanize())
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_static_method::<fn(_) -> _>("now", |()| Self::from(Utc::now()));
        registry.register_static_method::<fn(_, _) -> _>("parse", |ctx: CallContext, s: IStr| {
            s.parse::<DateTime<Utc>>()
                .map(Self::from)
                .map_err(|e| ctx.error(format!("Unable to parse DateTime: {}", e)))
        });
        registry.register_method::<fn(&mut _) -> _>("to_rfc3339", |this| {
            this.inner.to_rfc3339().intern()
        });

        registry.register_bin_op(BinOp::Add, |lhs, rhs: &DurationValue| {
            Self::from(lhs.inner + rhs.0)
        });
    }
}

#[cfg(test)]
mod test {
    use chrono::{TimeDelta, TimeZone, Utc};

    use crate::{
        eval,
        script::{base_engine, datetime::DateTimeValue},
    };

    #[test]
    fn now() {
        let e = base_engine();

        let d = eval!(e, DateTime.now());
        let unwrapped = d.unwrap::<DateTimeValue>().inner;
        assert!(unwrapped - Utc::now() < TimeDelta::seconds(5));
    }

    #[test]
    fn parse() {
        let e = base_engine();

        let d = eval!(e, DateTime.parse("2015-05-15T12:00:00Z"));
        let unwrapped = d.unwrap::<DateTimeValue>().inner;
        assert_eq!(
            unwrapped,
            Utc.with_ymd_and_hms(2015, 5, 15, 12, 0, 0).unwrap()
        );
    }

    #[test]
    fn add() {
        let e = base_engine();

        let d = eval!(
            e,
            DateTime.parse("2015-05-15T12:00:00Z") + Duration.parse("1m 30s")
        );
        let unwrapped = d.unwrap::<DateTimeValue>().inner;
        assert_eq!(
            unwrapped,
            Utc.with_ymd_and_hms(2015, 5, 15, 12, 1, 30).unwrap()
        );
    }
}
