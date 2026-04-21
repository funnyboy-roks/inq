use std::{borrow::Cow, collections::HashSet};

use miette::bail;

#[derive(Debug, Clone, Copy)]
pub struct Interpolated<'a>(&'a str);

impl<'a> From<&'a str> for Interpolated<'a> {
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

impl<'a> Interpolated<'a> {
    fn interpolate_inner<F>(
        self,
        get: &mut F,
        expanding: &mut HashSet<&'a str>,
    ) -> miette::Result<Cow<'a, str>>
    where
        F: FnMut(&'a str) -> Option<&'a str>,
    {
        if !self.0.contains("${") {
            return Ok(Cow::Borrowed(self.0));
        }

        let mut out = String::new();
        let mut rest = self.0;
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
                        bail!("Recursive expansion detected in {:?}", self.0);
                    }
                    rest = &rest[pos + 1..];
                    let var = get(var_name).ok_or_else(|| {
                        miette::miette!("Undefined variable '{}'", var_name.to_string())
                    })?;
                    expanding.insert(var_name);
                    let var = Self(var).interpolate_inner(get, expanding)?;
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

    pub(crate) fn interpolate<F>(self, mut get: F) -> miette::Result<Cow<'a, str>>
    where
        F: FnMut(&'a str) -> Option<&'a str>,
    {
        self.interpolate_inner(&mut get, &mut HashSet::new())
    }
}

#[cfg(test)]
mod test {
    use crate::util::Interpolated;
    use std::borrow::Cow;

    #[test]
    fn no_vars() {
        let s = Interpolated::from("hello world")
            .interpolate(|_| None)
            .unwrap();
        assert!(matches!(s, Cow::Borrowed(_)));
        assert_eq!(s, "hello world")
    }

    #[test]
    fn one_variable() {
        let s = Interpolated::from("hello ${LOC}")
            .interpolate(|n| (n == "LOC").then_some("world"))
            .unwrap();
        assert_eq!(s, "hello world")
    }

    #[test]
    fn two_variable() {
        let s = Interpolated::from("hello ${FOO} ${BAR} baz")
            .interpolate(|n| match n {
                "FOO" => Some("foo"),
                "BAR" => Some("bar"),
                _ => None,
            })
            .unwrap();
        assert_eq!(s, "hello foo bar baz")
    }

    #[test]
    fn repeat() {
        let s = Interpolated::from("hello ${FOO} ${FOO} baz")
            .interpolate(|n| match n {
                "FOO" => Some("foo"),
                _ => None,
            })
            .unwrap();
        assert_eq!(s, "hello foo foo baz")
    }

    #[test]
    fn recursive() {
        let s = Interpolated::from("http://${HOST}/login")
            .interpolate(|n| match n {
                "HOST" => Some("localhost:${PORT}"),
                "PORT" => Some("6969"),
                _ => None,
            })
            .unwrap();
        assert_eq!(s, "http://localhost:6969/login")
    }

    #[test]
    fn recursive_inf() {
        let s = Interpolated::from("http://${HOST}/login").interpolate(|n| match n {
            "HOST" => Some("localhost:${PORT}"),
            "PORT" => Some("${HOST}"),
            _ => None,
        });
        assert!(s.is_err())
    }

    #[test]
    fn recursive_self() {
        let s = Interpolated::from("http://${HOST}/login").interpolate(|n| match n {
            "HOST" => Some("localhost:${HOST}"),
            _ => None,
        });
        assert!(s.is_err())
    }
}
