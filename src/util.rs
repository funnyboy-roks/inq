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
        expanded: &mut HashSet<&'a str>,
    ) -> miette::Result<Cow<'a, str>>
    where
        F: FnMut(&'a str) -> Option<&'a str>,
    {
        if !self.0.contains("${") {
            return Ok(Cow::Borrowed(&self.0));
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
                    if expanded.contains(var_name) {
                        bail!("Recursive expansion detected in {:?}", self.0);
                    }
                    rest = &rest[pos + 1..];
                    let var = get(var_name).ok_or_else(|| {
                        miette::miette!("Undefined variable '{}'", var_name.to_string())
                    })?;
                    expanded.insert(var);
                    let var = Self(var).interpolate_inner(get, expanded)?;
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
    use crate::util::interpolate;

    #[test]
    fn no_vars() {
        let s = interpolate("hello world", |_| None).unwrap();
        assert_eq!(s, "hello world")
    }

    #[test]
    fn one_variable() {
        let s = interpolate("hello ${LOC}", |n| (n == "LOC").then_some("world")).unwrap();
        assert_eq!(s, "hello world")
    }

    #[test]
    fn two_variable() {
        let s = interpolate("hello ${FOO} ${BAR} baz", |n| match n {
            "FOO" => Some("foo"),
            "BAR" => Some("bar"),
            _ => None,
        })
        .unwrap();
        assert_eq!(s, "hello foo bar baz")
    }

    #[test]
    fn repeat() {
        let s = interpolate("hello ${FOO} ${FOO} baz", |n| match n {
            "FOO" => Some("foo"),
            _ => None,
        })
        .unwrap();
        assert_eq!(s, "hello foo foo baz")
    }
}
