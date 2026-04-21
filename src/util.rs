use std::{borrow::Cow, collections::HashSet};

use miette::{LabeledSpan, SourceSpan, bail};

#[derive(Debug, Clone, Copy)]
pub struct Interpolated<'a>(&'a str);

impl<'a> From<&'a str> for Interpolated<'a> {
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

impl Interpolated<'_> {
    fn interpolate_inner<'a, F>(
        s: Cow<'a, str>,
        get: &mut F,
        expanding: &mut HashSet<String>,
    ) -> miette::Result<Cow<'a, str>>
    where
        F: FnMut(&str) -> miette::Result<Option<String>>,
    {
        if !s.contains("${") {
            return Ok(s);
        }

        let mut out = String::new();
        let mut rest = &*s;
        while !rest.is_empty() {
            // ${VAR_NAME}
            if let Some(pos) = rest.find("${") {
                let pre = &rest[..pos];
                rest = &rest[pos + "${".len()..];
                // VAR_NAME}
                out.push_str(pre);
                if let Some(pos) = rest.find("}") {
                    let var_name = &rest[..pos];
                    if expanding.contains(var_name) {
                        bail!("Recursive expansion detected in {:?}", s);
                    }
                    rest = &rest[pos + 1..];
                    let ovar = get(var_name)?.ok_or_else(|| {
                        miette::miette!("Undefined variable '{}'", var_name.to_string())
                    })?;
                    expanding.insert(var_name.to_string());
                    let var = Self::interpolate_inner(Cow::Borrowed(&ovar), get, expanding)?;
                    expanding.remove(var_name);
                    out.push_str(&var);
                }
            } else {
                out.push_str(rest);
                break;
            }
        }
        Ok(Cow::Owned(out))
    }
}

impl<'a> Interpolated<'a> {
    pub(crate) fn interpolate<F>(self, mut get: F) -> miette::Result<Cow<'a, str>>
    where
        F: FnMut(&str) -> miette::Result<Option<String>>,
    {
        Self::interpolate_inner(Cow::Borrowed(self.0), &mut get, &mut HashSet::new())
    }
}

pub(crate) trait WithLabel {
    fn with_label(self, label: impl Into<String>) -> LabeledSpan;
}

impl WithLabel for SourceSpan {
    fn with_label(self, label: impl Into<String>) -> LabeledSpan {
        LabeledSpan::new_with_span(Some(label.into()), self)
    }
}

#[cfg(test)]
mod test {
    use crate::util::Interpolated;
    use std::borrow::Cow;

    #[test]
    fn no_vars() {
        let s = Interpolated::from("hello world")
            .interpolate(|_| Ok(None))
            .unwrap();
        assert!(matches!(s, Cow::Borrowed(_)));
        assert_eq!(s, "hello world")
    }

    #[test]
    fn one_variable() {
        let s = Interpolated::from("hello ${LOC}")
            .interpolate(|n| Ok((n == "LOC").then_some("world".to_string())))
            .unwrap();
        assert_eq!(s, "hello world")
    }

    #[test]
    fn two_variable() {
        let s = Interpolated::from("hello ${FOO} ${BAR} baz")
            .interpolate(|n| {
                Ok(match n {
                    "FOO" => Some("foo".into()),
                    "BAR" => Some("bar".into()),
                    _ => None,
                })
            })
            .unwrap();
        assert_eq!(s, "hello foo bar baz")
    }

    #[test]
    fn repeat() {
        let s = Interpolated::from("hello ${FOO} ${FOO} baz")
            .interpolate(|n| {
                Ok(match n {
                    "FOO" => Some("foo".into()),
                    _ => None,
                })
            })
            .unwrap();
        assert_eq!(s, "hello foo foo baz")
    }

    #[test]
    fn recursive() {
        let s = Interpolated::from("http://${HOST}/login")
            .interpolate(|n| {
                Ok(match n {
                    "HOST" => Some("localhost:${PORT}".into()),
                    "PORT" => Some("6969".into()),
                    _ => None,
                })
            })
            .unwrap();
        assert_eq!(s, "http://localhost:6969/login")
    }

    #[test]
    fn recursive_inf() {
        let s = Interpolated::from("http://${HOST}/login").interpolate(|n| {
            Ok(match n {
                "HOST" => Some("localhost:${PORT}".into()),
                "PORT" => Some("${HOST}".into()),
                _ => None,
            })
        });
        assert!(s.is_err())
    }

    #[test]
    fn recursive_self() {
        let s = Interpolated::from("http://${HOST}/login").interpolate(|n| {
            Ok(match n {
                "HOST" => Some("localhost:${HOST}".into()),
                _ => None,
            })
        });
        assert!(s.is_err())
    }
}
