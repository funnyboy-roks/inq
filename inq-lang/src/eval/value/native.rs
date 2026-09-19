use std::{borrow::Cow, cell::RefCell, cmp::Ordering, fmt::Debug, rc::Rc, str::FromStr};

use indexmap::IndexMap;

use crate::{
    Span,
    eval::{
        EvalError, EvalResult,
        registry::{BinOp, PrefixOp, Registry, UnaryOp, VarArgs},
        value::CallContext,
    },
    parse::Ident,
    string::IStr,
};

use super::{Value, ValueRef};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Null;
impl From<()> for Null {
    fn from((): ()) -> Self {
        Self
    }
}

impl From<()> for ValueRef {
    fn from((): ()) -> Self {
        ValueRef::null()
    }
}

fn to_string_radix(mut n: i64, radix: i64) -> IStr {
    debug_assert!((2..=36).contains(&radix));

    let mut s = String::new();
    if n < 0 {
        s.push('-');
        n = -n;
    }

    fn map(c: i64) -> char {
        char::from(match c {
            0..=9 => c as u8 + b'0',
            10..36 => (c - 10) as u8 + b'a',
            _ => unreachable!(),
        })
    }

    loop {
        s.insert(0, map(n % radix));
        n /= radix;
        if n <= 0 {
            break;
        }
    }

    s.into()
}

pub type Int = i64;
impl Value for Int {
    fn type_name() -> Cow<'static, str> {
        "Int".into()
    }
    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self).expect("Write to string can't fail");
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, fmt)
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        unreachable!()
    }
    fn truthy(&self) -> bool {
        *self != 0
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_bin_op(BinOp::Add, |_, &lhs, &rhs: &Self| lhs + rhs);
        registry.register_bin_op(BinOp::Add, |_, &lhs, &rhs: &Float| lhs as Float + rhs);
        registry.register_bin_op(BinOp::Add, |_, &lhs, rhs: &IStr| {
            IStr::from(format!("{}{}", lhs, rhs))
        });

        registry.register_bin_op(BinOp::Sub, |_, &lhs, &rhs: &Self| lhs - rhs);
        registry.register_bin_op(BinOp::Sub, |_, &lhs, &rhs: &Float| lhs as Float - rhs);

        registry.register_bin_op(BinOp::Mul, |_, &lhs, &rhs: &Self| lhs * rhs);
        registry.register_bin_op(BinOp::Mul, |_, &lhs, &rhs: &Float| lhs as Float * rhs);

        registry.register_bin_op(BinOp::Div, |ctx, &lhs, &rhs: &Self| {
            lhs.checked_div(rhs)
                .ok_or(EvalError::Div0 { span: ctx.span() })
        });
        registry.register_bin_op(BinOp::Div, |_, &lhs, &rhs: &Float| lhs as Float / rhs);

        registry.register_unary_op(UnaryOp::Prefix(PrefixOp::Neg), |_, &i| -i);

        registry.register_cmp(|l, r| l.partial_cmp(r));
        registry.register_cmp(|l, r| (*l as Float).partial_cmp(r));

        registry.register_method::<fn(_, &_, _) -> _>(
            "to_string",
            |ctx, &this, radix: Option<i64>| {
                let radix = radix.unwrap_or(10.into());
                if !(2..=36).contains(&radix) {
                    return Err(ctx.error(format!("radix must be in range [2, 36], got {}", radix)));
                }
                Ok(to_string_radix(this, radix))
            },
        );
        registry.register_static_method::<fn(_, _) -> _>("parse", |ctx: CallContext, s: IStr| {
            Int::from_str(&s).map_err(|e| ctx.error(format!("Cannot parse {:?} as Int: {}", s, e)))
        });

        registry.register_method::<fn(&_) -> _>("to_float", |&this| this as Float);
    }
}

pub type Float = f64;
impl Value for Float {
    fn type_name() -> Cow<'static, str> {
        "Float".into()
    }
    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self).expect("Write to string can't fail");
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, fmt)
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        unreachable!()
    }
    fn truthy(&self) -> bool {
        *self != 0.0
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_bin_op(BinOp::Add, |_, &lhs, &rhs: &Self| lhs + rhs);
        registry.register_bin_op(BinOp::Add, |_, &lhs, &rhs: &Int| lhs + rhs as Float);
        registry.register_bin_op(BinOp::Add, |_, &lhs, rhs: &IStr| {
            IStr::from(format!("{}{}", lhs, rhs))
        });

        registry.register_bin_op(BinOp::Sub, |_, &lhs, &rhs: &Self| lhs - rhs);
        registry.register_bin_op(BinOp::Sub, |_, &lhs, &rhs: &Int| lhs - rhs as Float);

        registry.register_bin_op(BinOp::Mul, |_, &lhs, &rhs: &Self| lhs * rhs);
        registry.register_bin_op(BinOp::Mul, |_, &lhs, &rhs: &Int| lhs * rhs as Float);

        registry.register_bin_op(BinOp::Div, |ctx, &lhs, &rhs: &Self| {
            if rhs == 0.0 {
                Err(EvalError::Div0 { span: ctx.span() })
            } else {
                Ok(lhs / rhs)
            }
        });
        registry.register_bin_op(BinOp::Div, |_, &lhs, &rhs: &Int| lhs / rhs as Float);

        registry.register_unary_op(UnaryOp::Prefix(PrefixOp::Neg), |_, &i| -i);

        registry.register_cmp(|l, r| l.partial_cmp(r));

        macro_rules! proxy {
            ($($fun: ident)*) => {
                $(
                registry.register_method::<fn(&_) -> _>(stringify!($fun), |&this| this.$fun());
                )*
            };
        }
        proxy!(floor ceil round trunc fract sqrt exp exp2 ln log2 log10 cbrt sin cos);

        registry.register_static_method::<fn(_, _) -> _>("parse", |ctx: CallContext, s: IStr| {
            Self::from_str(&s)
                .map_err(|e| ctx.error(format!("Cannot parse {:?} as Float: {}", s, e)))
        });
    }
}

impl Value for bool {
    fn type_name() -> Cow<'static, str> {
        "Bool".into()
    }
    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "{}", self).expect("Write to string can't fail");
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, fmt)
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        unreachable!()
    }
    fn truthy(&self) -> bool {
        *self
    }

    fn register(_: &mut Registry<Self>)
    where
        Self: Sized,
    {
    }
}

impl Value for Null {
    fn type_name() -> Cow<'static, str> {
        "Null".into()
    }
    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn to_string(&self, out: &mut String) {
        out.push_str("null");
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "null")
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        unreachable!()
    }
    fn truthy(&self) -> bool {
        false
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_cmp(|&Null, v: &ValueRef| v.downcast::<Null>().map(|_| Ordering::Equal));
    }
}

impl Value for IStr {
    fn type_name() -> Cow<'static, str> {
        "String".into()
    }
    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn to_string(&self, out: &mut String) {
        out.push_str(self);
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, fmt)
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        unreachable!()
    }
    fn truthy(&self) -> bool {
        !self.is_empty()
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&_) -> _>("len", |s| s.len() as i64);
        registry.register_method::<fn(&_, IStr) -> _>("split", |this, delim| {
            this.split(delim.as_str())
                .map(IStr::from)
                .map(ValueRef::from)
                .collect::<Array>()
        });
        registry.register_method::<fn(&_, _) -> _>(
            "replace",
            |this, (needle, replacement): (IStr, IStr)| -> IStr {
                this.replace(&*needle, &replacement).into()
            },
        );
        registry.register_method::<fn(_, &_, _) -> _>(
            "substring",
            |ctx, this, (start, end): (i64, i64)| {
                let nstart = if start < 0 {
                    this.len() as i64 + start
                } else {
                    start
                };
                let nend = if end < 0 {
                    this.len() as i64 + end
                } else {
                    end
                };

                if nstart < 0 || nstart > this.len() as i64 {
                    return Err(ctx.error(format!(
                        "Start {} out of bounds for length {}",
                        start,
                        this.len()
                    )));
                }
                if nend < 0 || nend > this.len() as i64 {
                    return Err(ctx.error(format!(
                        "End {} out of bounds for length {}",
                        end,
                        this.len()
                    )));
                }

                Ok(IStr::from(&this[start as usize..end as usize]))
            },
        );
        registry.register_method::<fn(&_) -> _>("chars", |this| {
            this.chars()
                .map(IStr::from)
                .map(ValueRef::from)
                .collect::<Array>()
        });

        registry.register_cmp(|lhs, rhs: &IStr| lhs.partial_cmp(rhs));

        registry.register_bin_op(BinOp::Add, |_, lhs, rhs: &ValueRef| {
            let mut out = String::from(lhs);
            rhs.value().to_string(&mut out);
            IStr::from(out)
        });
    }
}

/// Array
#[derive(Debug, Clone)]
pub struct Array(RefCell<Vec<ValueRef>>);

impl<V: Into<ValueRef>> FromIterator<V> for Array {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Self(RefCell::new(iter.into_iter().map(Into::into).collect()))
    }
}

impl From<Array> for Vec<ValueRef> {
    fn from(value: Array) -> Self {
        value.0.into_inner()
    }
}

impl Array {
    pub fn into_vec(self) -> Vec<ValueRef> {
        self.into()
    }
}

impl Value for Array {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized,
    {
        "Array".into()
    }

    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        out.push('[');
        for (i, x) in self.0.borrow().iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            x.value().to_string(out)
        }
        out.push(']');
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_list()
            .entries(
                self.0
                    .borrow()
                    .iter()
                    .map(|e| std::fmt::from_fn(|fmt| e.value().debug(fmt))),
            )
            .finish()
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(
            self.0
                .borrow()
                .iter()
                .map(|x| x.snapshot())
                .collect::<Self>(),
        )
    }

    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&_) -> _>("len", |this| this.0.borrow().len() as i64);
        registry.register_method::<fn(&_, VarArgs) -> _>("push", |this, args| {
            this.0.borrow_mut().extend(args.inner);
            ValueRef::null()
        });
        registry.register_index_get_set(
            |_, this, &idx: &i64| {
                let this = this.0.borrow();
                Ok(normalise_index(idx, this.len())?.map(|n| this[n].clone()))
            },
            |ctx, this: &Self, &idx: &i64, value: ValueRef| {
                let mut this = this.0.borrow_mut();
                let idx = normalise_index_error(idx, this.len(), ctx.span)?;
                this[idx] = value;
                Ok(())
            },
        );
    }
}

#[derive(Debug, Clone)]
pub struct Object(pub RefCell<IndexMap<IStr, ValueRef>>);

impl Object {
    pub fn into_map(self) -> IndexMap<IStr, ValueRef> {
        self.0.into_inner()
    }
}

impl Value for Object {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized,
    {
        "Object".into()
    }

    fn type_name_of(&self) -> Cow<'static, str> {
        "Object".into()
    }

    fn to_string(&self, out: &mut String) {
        out.push('{');
        for (k, v) in &*self.0.borrow() {
            if Ident::is_valid(k) {
                out.push_str(k);
            } else {
                out.push('"');
                for c in k.chars() {
                    if c == '"' {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('"');
            }
            out.push_str(": ");
            v.value().to_string(out);
        }
        out.push('}');
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_map()
            .entries(self.0.borrow().iter().map(|(k, v)| (k, v.debug())))
            .finish()
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(
            self.0
                .borrow()
                .iter()
                .map(|(k, v)| (k.clone(), v.snapshot()))
                .collect::<Self>(),
        )
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_index_get_set(
            |_, this, idx: &IStr| this.0.borrow().get(&**idx).cloned(),
            |_, this, idx: &IStr, value| {
                this.0.borrow_mut().insert(idx.clone(), value);
            },
        );
        registry.register_field_get_set_fallback(
            |_, this, field| {
                if let Some(value) = this.0.borrow().get(&field.inner).cloned() {
                    Ok(value)
                } else {
                    Err(EvalError::UnknownField {
                        ty: this.type_name_of().into(),
                        field,
                    })
                }
            },
            |_, this, field, value| {
                this.0.borrow_mut().insert(field.inner, value);
            },
        );
    }
}

impl<K: Into<IStr>, V: Into<ValueRef>> FromIterator<(K, V)> for Object {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self(RefCell::new(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        ))
    }
}

impl Value for fn(&IStr) -> ValueRef {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized,
    {
        "Function".into()
    }
    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn to_string(&self, out: &mut String) {
        out.push_str("<native function>")
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "<native function>")
    }
    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(*self)
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_call::<fn(&_, IStr) -> _>(|this, s| this(&s));
    }
}

pub fn normalise_index(i: i64, len: usize) -> EvalResult<Option<usize>> {
    let ni = if i < 0 { len as i64 + i } else { i };

    if ni < 0 || ni >= len as i64 {
        Ok(None)
    } else {
        Ok(Some(ni as _))
    }
}

pub fn normalise_index_error(i: i64, len: usize, span: Span) -> EvalResult<usize> {
    normalise_index(i, len)?.ok_or_else(|| EvalError::Custom {
        message: format!("Index {} out of bounds for length {}", i, len),
        span,
    })
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use crate::{Span, eval::value::native::normalise_index_error};

    #[test]
    fn norm_index() {
        assert_matches!(normalise_index_error(-5, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index_error(-4, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index_error(-3, 3, Span::empty()), Ok(0));
        assert_matches!(normalise_index_error(-2, 3, Span::empty()), Ok(1));
        assert_matches!(normalise_index_error(-1, 3, Span::empty()), Ok(2));
        assert_matches!(normalise_index_error(0, 3, Span::empty()), Ok(0));
        assert_matches!(normalise_index_error(1, 3, Span::empty()), Ok(1));
        assert_matches!(normalise_index_error(2, 3, Span::empty()), Ok(2));
        assert_matches!(normalise_index_error(3, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index_error(4, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index_error(5, 3, Span::empty()), Err(_));
    }
}
