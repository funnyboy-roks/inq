use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use miette::{LabeledSpan, SourceSpan};
use rhai::{Dynamic, EvalAltResult};

use crate::script::base_engine;

pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone)]
pub struct Interpolated<'a>(Cow<'a, str>);

impl<'a> From<Cow<'a, str>> for Interpolated<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        Self(value)
    }
}

impl From<String> for Interpolated<'static> {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl<'a> From<&'a str> for Interpolated<'a> {
    fn from(value: &'a str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

enum InterpolateInnerError {
    Miette(miette::Report),
    Rhai(Box<EvalAltResult>),
}

impl From<miette::Report> for InterpolateInnerError {
    fn from(value: miette::Report) -> Self {
        Self::Miette(value)
    }
}

impl From<Box<EvalAltResult>> for InterpolateInnerError {
    fn from(value: Box<EvalAltResult>) -> Self {
        Self::Rhai(value)
    }
}

impl From<EvalAltResult> for InterpolateInnerError {
    fn from(value: EvalAltResult) -> Self {
        Self::Rhai(Box::new(value))
    }
}

impl Interpolated<'_> {
    pub fn to_owned(&self) -> Interpolated<'static> {
        Interpolated(Cow::Owned(self.0.clone().into_owned()))
    }

    fn interpolate_inner<'a>(
        s: Cow<'a, str>,
        vars: &HashMap<String, Interpolated<'a>>,
        expanding: &HashSet<String>,
    ) -> Result<Cow<'a, str>, InterpolateInnerError> {
        if !s.contains("${") {
            return Ok(s);
        }

        let mut out = String::new();
        let mut rest = &*s;
        while !rest.is_empty() {
            if !rest.contains("${") {
                out.push_str(rest);
                return Ok(Cow::Owned(out));
            }

            // ${VAR_NAME}
            if let Some(pos) = rest.find("${") {
                let pre = &rest[..pos];
                rest = &rest[pos + "${".len()..];

                let mut depth = 1;

                out.push_str(pre);
                let expr = rest;
                while depth > 0 {
                    let next_close = rest.find("}");
                    let next_open = rest.find("{");

                    match (next_close, next_open) {
                        (Some(c), Some(o)) => {
                            if c < o {
                                // close before open
                                rest = &rest[c + 1..];
                                depth -= 1;
                            } else if o < c {
                                // open before close
                                rest = &rest[o + 1..];
                                depth += 1;
                            } else {
                                unreachable!("Different patterns can not be in the same position.");
                            }
                        }
                        (Some(c), None) => {
                            rest = &rest[c + 1..];
                            depth -= 1;
                        }
                        (None, _) => {
                            return Err(miette::miette!("Unclosed ${{ in {:?}", s).into());
                        }
                    }
                }
                let expr = expr
                    .strip_suffix(rest)
                    .expect("rest is a substring of expr");
                let expr = &expr[..expr.len() - 1]; // strip trailing }

                if expanding.contains(expr) {
                    return Err(miette::miette!("Recursive expansion detected in {:?}", s).into());
                }

                let mut engine = base_engine();

                let vars: HashMap<String, Interpolated<'static>> = vars
                    .iter()
                    .map(|(name, val)| (name.clone(), val.to_owned()))
                    .collect::<HashMap<_, _>>();

                let expanding = expanding.clone();
                #[allow(deprecated, reason = "Volatile, but we need it")]
                engine.on_var(move |name, _index, _ctx| {
                    let mut expanding = expanding.clone();
                    if let Some(var) = vars.get(name) {
                        expanding.insert(name.to_string());
                        let var = Self::interpolate_inner(var.0.clone(), &vars, &expanding);
                        let var = match var {
                            Ok(var) => var,
                            Err(InterpolateInnerError::Miette(e)) => {
                                return Err(Box::new(EvalAltResult::ErrorSystem(
                                    "miette error".to_string(),
                                    Box::new(std::io::Error::other(e)),
                                )));
                            }
                            Err(InterpolateInnerError::Rhai(e)) => return Err(e),
                        };
                        expanding.remove(name);
                        Ok(Some(var.into_owned().into()))
                    } else {
                        Ok(None)
                    }
                });

                let var: Dynamic = engine.eval_expression(expr)?;
                let var = var.to_string();

                out.push_str(&var);
            } else {
                out.push_str(rest);
                break;
            }
        }
        Ok(Cow::Owned(out))
    }
}

impl<'a> Interpolated<'a> {
    pub(crate) fn interpolate(
        &self,
        vars: &HashMap<String, Interpolated<'a>>,
    ) -> miette::Result<Cow<'a, str>> {
        match Self::interpolate_inner(self.0.clone(), vars, &HashSet::new()) {
            Ok(v) => Ok(v),
            Err(InterpolateInnerError::Miette(r)) => Err(r),
            Err(InterpolateInnerError::Rhai(e)) => {
                Err(miette::miette!("Rhai error evaluating {:?}: {}", self.0, e))
            }
        }
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
    use std::{borrow::Cow, collections::HashMap};

    macro_rules! var_map {
        {$($name: literal => $value: literal),*$(,)?} => {
             HashMap::from_iter([
                 $(($name.into(), $value.into())),*
             ])
        };
    }

    #[test]
    fn no_vars() {
        let s = Interpolated::from("hello world")
            .interpolate(&var_map! {})
            .unwrap();
        assert!(matches!(s, Cow::Borrowed(_)));
        assert_eq!(s, "hello world")
    }

    #[test]
    fn one_variable() {
        let s = Interpolated::from("hello ${LOC}")
            .interpolate(&var_map! {
                "LOC" => "world",
            })
            .unwrap();
        assert_eq!(s, "hello world")
    }

    #[test]
    fn two_variable() {
        let s = Interpolated::from("hello ${FOO} ${BAR} baz")
            .interpolate(&var_map! {
                "FOO" => "foo",
                "BAR" => "bar",
            })
            .unwrap();
        assert_eq!(s, "hello foo bar baz")
    }

    #[test]
    fn repeat() {
        let s = Interpolated::from("hello ${FOO} ${FOO} baz")
            .interpolate(&var_map! {
                "FOO" => "foo",
            })
            .unwrap();
        assert_eq!(s, "hello foo foo baz")
    }

    #[test]
    fn recursive() {
        let s = Interpolated::from("http://${HOST}/login")
            .interpolate(&var_map! {
                "HOST" => "localhost:${PORT}",
                "PORT" => "6969",
            })
            .unwrap();
        assert_eq!(s, "http://localhost:6969/login")
    }

    #[test]
    fn recursive_inf() {
        let s = Interpolated::from("http://${HOST}/login").interpolate(&var_map! {
            "HOST" => "localhost:${PORT}",
            "PORT" => "${HOST}",
        });
        assert!(s.is_err())
    }

    #[test]
    fn recursive_self() {
        let s = Interpolated::from("http://${HOST}/login").interpolate(&var_map! {
            "HOST" => "localhost:${HOST}",
        });
        assert!(s.is_err())
    }
}
