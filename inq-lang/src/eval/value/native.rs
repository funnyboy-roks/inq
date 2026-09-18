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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(*self))
    }
    fn truthy(&self) -> bool {
        *self != 0
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_bin_op(BinOp::Add, |&lhs, &rhs: &Self| lhs + rhs);
        registry.register_bin_op(BinOp::Add, |&lhs, &rhs: &Float| lhs as Float + rhs);
        registry.register_bin_op(BinOp::Add, |&lhs, rhs: &IStr| {
            IStr::from(format!("{}{}", lhs, rhs))
        });

        registry.register_bin_op(BinOp::Sub, |&lhs, &rhs: &Self| lhs - rhs);
        registry.register_bin_op(BinOp::Sub, |&lhs, &rhs: &Float| lhs as Float - rhs);

        registry.register_bin_op(BinOp::Mul, |&lhs, &rhs: &Self| lhs * rhs);
        registry.register_bin_op(BinOp::Mul, |&lhs, &rhs: &Float| lhs as Float * rhs);

        // TODO: div 0
        registry.register_bin_op(BinOp::Div, |&lhs, &rhs: &Self| lhs / rhs);
        registry.register_bin_op(BinOp::Div, |&lhs, &rhs: &Float| lhs as Float / rhs);

        registry.register_unary_op(UnaryOp::Prefix(PrefixOp::Neg), |_, &i| -i);

        registry.register_cmp(|l, r| l.partial_cmp(r));
        registry.register_cmp(|l, r| (*l as Float).partial_cmp(r));

        registry.register_method::<fn(_, &mut _, _) -> _>(
            "to_string",
            |ctx, &mut this, radix: Option<i64>| {
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

        registry.register_method::<fn(&mut _) -> _>("to_float", |&mut this| this as Float);
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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(*self))
    }
    fn truthy(&self) -> bool {
        *self != 0.0
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_bin_op(BinOp::Add, |&lhs, &rhs: &Self| lhs + rhs);
        registry.register_bin_op(BinOp::Add, |&lhs, &rhs: &Int| lhs + rhs as Float);
        registry.register_bin_op(BinOp::Add, |&lhs, rhs: &IStr| {
            IStr::from(format!("{}{}", lhs, rhs))
        });

        registry.register_bin_op(BinOp::Sub, |&lhs, &rhs: &Self| lhs - rhs);
        registry.register_bin_op(BinOp::Sub, |&lhs, &rhs: &Int| lhs - rhs as Float);

        registry.register_bin_op(BinOp::Mul, |&lhs, &rhs: &Self| lhs * rhs);
        registry.register_bin_op(BinOp::Mul, |&lhs, &rhs: &Int| lhs * rhs as Float);

        // TODO: div 0
        registry.register_bin_op(BinOp::Div, |&lhs, &rhs: &Self| lhs / rhs);
        registry.register_bin_op(BinOp::Div, |&lhs, &rhs: &Int| lhs / rhs as Float);

        registry.register_unary_op(UnaryOp::Prefix(PrefixOp::Neg), |_, &i| -i);

        registry.register_cmp(|l, r| l.partial_cmp(r));

        macro_rules! proxy {
            ($($fun: ident)*) => {
                $(
                registry.register_method::<fn(&mut _) -> _>(stringify!($fun), |&mut this| this.$fun());
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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(*self))
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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }
    fn truthy(&self) -> bool {
        !self.is_empty()
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&mut _) -> _>("len", |s| s.len() as i64);
        registry.register_method::<fn(&mut _, IStr) -> _>("split", |this, delim| {
            this.split(delim.as_str())
                .map(IStr::from)
                .map(ValueRef::from)
                .collect::<Vec<_>>()
        });
        registry.register_method::<fn(&mut _, _) -> _>(
            "replace",
            |this, (needle, replacement): (IStr, IStr)| -> IStr {
                this.replace(&*needle, &replacement).into()
            },
        );
        registry.register_method::<fn(_, &mut _, _) -> _>(
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
        registry.register_method::<fn(&mut _) -> _>("chars", |this| {
            this.chars()
                .map(IStr::from)
                .map(ValueRef::from)
                .collect::<Array>()
        });

        registry.register_cmp(|lhs, rhs: &IStr| lhs.partial_cmp(rhs));

        registry.register_bin_op(BinOp::Add, |lhs, rhs: &ValueRef| {
            let mut out = String::from(lhs);
            rhs.borrow().to_string(&mut out);
            IStr::from(out)
        });
    }
}

/// Array
pub type Array = Vec<ValueRef>;
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
        for (i, x) in self.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            x.borrow().to_string(out)
        }
        out.push(']');
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_list()
            .entries(
                self.iter()
                    .map(|e| std::fmt::from_fn(|fmt| e.borrow().debug(fmt))),
            )
            .finish()
    }
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(
            self.iter().map(|x| x.snapshot()).collect::<Self>(),
        ))
    }

    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_method::<fn(&mut _) -> _>("len", |this| this.len() as i64);
        registry.register_method::<fn(&mut _, VarArgs) -> _>("push", |this, args| {
            this.extend(args.inner);
            ValueRef::null()
        });
        registry.register_index_get_set(
            |ctx, this, &idx: &i64| Ok(this[normalise_index(idx, this.len(), ctx.span)?].clone()),
            |ctx, this: &mut Self, &idx: &i64, value: ValueRef| {
                let idx = normalise_index(idx, this.len(), ctx.span)?;
                this[idx] = value;
                Ok(())
            },
        );
    }
}

#[derive(Debug, Clone)]
pub struct Object(pub IndexMap<IStr, ValueRef>);
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
        for (k, v) in &self.0 {
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
            v.borrow().to_string(out);
        }
        out.push('}');
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_map()
            .entries(
                self.0
                    .iter()
                    .map(|(k, v)| (k, std::fmt::from_fn(|fmt| v.borrow().debug(fmt)))),
            )
            .finish()
    }

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(
            self.0
                .iter()
                .map(|(k, v)| (k.clone(), v.snapshot()))
                .collect::<Self>(),
        ))
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_index_get_set(
            |_, this, idx: &IStr| this.0.get(&**idx).cloned().unwrap_or_else(ValueRef::null),
            |_, this, idx: &IStr, value| {
                this.0.insert(idx.clone(), value);
            },
        );
        registry.register_field_get_set_fallback(
            |_, this, field| {
                if let Some(value) = this.0.get(&field.inner).cloned() {
                    Ok(value)
                } else {
                    Err(EvalError::UnknownField {
                        ty: this.type_name_of().into(),
                        field,
                    })
                }
            },
            |_, this, field, value| {
                this.0.insert(field.inner, value);
            },
        );
    }
}

impl<K: Into<IStr>, V: Into<ValueRef>> FromIterator<(K, V)> for Object {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
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
    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(*self))
    }
    fn truthy(&self) -> bool {
        true
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_call::<fn(&mut _, IStr) -> _>(|this, s| this(&s));
    }
}

pub fn normalise_index(i: i64, len: usize, span: Span) -> EvalResult<usize> {
    let ni = if i < 0 { len as i64 + i } else { i };

    if ni < 0 || ni >= len as i64 {
        Err(EvalError::Custom {
            message: format!("Index {} out of bounds for length {}", i, len),
            span,
        })
    } else {
        Ok(ni as _)
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use crate::{Span, eval::value::native::normalise_index};

    #[test]
    fn norm_index() {
        assert_matches!(normalise_index(-5, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index(-4, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index(-3, 3, Span::empty()), Ok(0));
        assert_matches!(normalise_index(-2, 3, Span::empty()), Ok(1));
        assert_matches!(normalise_index(-1, 3, Span::empty()), Ok(2));
        assert_matches!(normalise_index(0, 3, Span::empty()), Ok(0));
        assert_matches!(normalise_index(1, 3, Span::empty()), Ok(1));
        assert_matches!(normalise_index(2, 3, Span::empty()), Ok(2));
        assert_matches!(normalise_index(3, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index(4, 3, Span::empty()), Err(_));
        assert_matches!(normalise_index(5, 3, Span::empty()), Err(_));
    }
}
