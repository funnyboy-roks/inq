use std::{borrow::Cow, collections::HashMap, path::PathBuf, str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use kdl::{KdlDocument, KdlNode};
use miette::{Context, IntoDiagnostic, SourceSpan, bail};
use reqwest::{
    Method,
    blocking::{Client, Request},
};
use serde_json::Value as JsonValue;

use crate::{
    cli::SubCmd,
    state::State,
    util::{Interpolated, WithLabel},
};

#[derive(Debug, Clone)]
pub enum Body<'a> {
    Text(Interpolated<'a>),
    Json {
        json: Interpolated<'a>,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone)]
pub enum PopulatedBody<'a> {
    Text(Cow<'a, str>),
    Json(JsonValue),
}

#[derive(Debug, Clone)]
pub struct Query<'a> {
    _name: &'a str,
    method: Method,
    url: Interpolated<'a>,
    body: Option<Body<'a>>,
    headers: HashMap<&'a str, Interpolated<'a>>,
}

impl<'a> Query<'a> {
    pub fn from_node(node: &'a KdlNode) -> miette::Result<Option<Self>> {
        let name = node.name().value();

        let method = Method::from_bytes(
            node.entry(0)
                .context("Expected query method")?
                .value()
                .as_string()
                .context("Query method must be a string.")?
                .as_bytes(),
        )
        .into_diagnostic()
        .context("Invalid query method")?;

        let url = node
            .entry(1)
            .context("Expected query url")?
            .value()
            .as_string()
            .context("Query url must be a string.")?;

        let body = if let Some(children) = node.children()
            && let Some(body_node) = children.get("body")
        {
            match body_node.entries() {
                [entry]
                    if let Some(name) = entry.name()
                        && name.value() == "text" =>
                {
                    let text = entry
                        .value()
                        .as_string()
                        .context("Expected body.text to be a string.")?;
                    Some(Body::Text(text.into()))
                }
                [entry]
                    if let Some(name) = entry.name()
                        && name.value() == "json" =>
                {
                    Some(Body::Json {
                        json: entry
                            .value()
                            .as_string()
                            .context("Expected body.json to be a string.")?
                            .into(),
                        span: entry.span(),
                    })
                }
                _ => {
                    return Err(miette::miette! {
                        labels = vec![body_node.span().with_label("here")],
                        "Malformed `body` node",
                    });
                }
            }
        } else {
            None
        };

        let mut headers = HashMap::new();
        if let Some(children) = node.children()
            && let Some(node) = children.get("headers")
        {
            for n in node.iter_children() {
                let value = n
                    .entry(0)
                    .context("Expected <name> <value> headers.")?
                    .value()
                    .as_string()
                    .context("Expected header value to be a string.")?;

                headers.insert(n.name().value(), value.into());
            }
        };

        Ok(Some(Self {
            _name: name,
            method,
            url: url.into(),
            body,
            headers,
        }))
    }

    pub(crate) fn to_request(
        &self,
        client: &Client,
        vars: &Variables<'a>,
    ) -> miette::Result<(Request, Option<PopulatedBody<'a>>)> {
        let url = self.url.interpolate(vars).context("Interpolating URL")?;

        let mut builder = client.request(self.method.clone(), &*url);

        let populated_body = if let Some(body) = &self.body {
            match body {
                Body::Text(t) => {
                    let s = t.interpolate(vars)?;
                    builder = builder.body(s.clone().into_owned());
                    Some(PopulatedBody::Text(s))
                }
                Body::Json { json, span } => {
                    let interpolated = &json.interpolate(vars)?;
                    let json = serde_json::Value::from_str(interpolated).map_err(|e| {
                        miette::miette! {
                            labels = vec![span.with_label("in this JSON")],
                            "JSON Error: {}",
                            e
                        }
                    })?;

                    builder = builder.json(&json);
                    Some(PopulatedBody::Json(json))
                }
            }
        } else {
            None
        };

        for (&k, v) in &self.headers {
            builder = builder.header(k, &*v.interpolate(vars)?);
        }

        Ok((builder.build().into_diagnostic()?, populated_body))
    }
}

#[derive(Debug, Clone)]
pub enum VariableValue {
    Str(String),
    File(PathBuf),
    Env { var: String, span: SourceSpan },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persist {
    Never,
    Duration(Duration),
    Forever,
}

impl Persist {
    pub fn persists(self) -> bool {
        match self {
            Persist::Never => false,
            Persist::Duration(_) | Persist::Forever => true,
        }
    }

    pub fn duration(self) -> Option<Duration> {
        match self {
            Persist::Never | Persist::Forever => None,
            Persist::Duration(d) => Some(d),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub persist: Persist,
    pub default_value: VariableValue,
}

impl Variable {
    fn parse(node: &KdlNode) -> miette::Result<(String, Self)> {
        let name = node.name().value().to_string();

        let mut persist = Persist::Never;
        let mut value: Option<VariableValue> = None;
        for entry in node.entries() {
            if let Some(ename) = entry.name() {
                match ename.value() {
                    "file" => {
                        if value.is_some() {
                            bail! {
                                labels = vec![entry.span().with_label("here")],
                                "Default variable value may only be set once"
                            }
                        }

                        let Some(s) = entry.value().as_string() else {
                            bail! {
                                labels = vec![entry.span().with_label("here")],
                                "File path must be a string"
                            }
                        };

                        let path = std::env::current_dir().into_diagnostic()?.join(s);

                        value = Some(VariableValue::File(path));
                    }
                    "env" => {
                        if value.is_some() {
                            bail! {
                                labels = vec![entry.span().with_label("here")],
                                "Default variable value may only be set once"
                            }
                        }

                        let Some(s) = entry.value().as_string() else {
                            bail! {
                                labels = vec![entry.span().with_label("here")],
                                "Environment variable must be a string"
                            }
                        };

                        value = Some(VariableValue::Env {
                            var: s.into(),
                            span: entry.span(),
                        });
                    }
                    "persist" => {
                        if persist != Persist::Never {
                            bail! {
                                labels = vec![entry.span().with_label("here")],
                                "Persist may only be set once"
                            }
                        }

                        if let Some(b) = entry.value().as_bool() {
                            if !b {
                                bail! {
                                    labels = vec![entry.span().with_label("here")],
                                    "Persist must either be #true or a duration"
                                }
                            }
                            persist = Persist::Forever;
                        } else if let Some(s) = entry.value().as_string() {
                            match humantime::parse_duration(s) {
                                Ok(d) => {
                                    persist = Persist::Duration(d);
                                }
                                Err(e) => {
                                    bail! {
                                        labels = vec![entry.span().with_label("here")],
                                        "Duration Parse Error: {}", e
                                    }
                                }
                            }
                        } else {
                            bail! {
                                labels = vec![entry.span().with_label("here")],
                                "Persist must either be #true or a duration like \"1 hour\""
                            }
                        }
                    }
                    _ => {
                        bail! {
                            labels = vec![entry.span().with_label("here")],
                            "Expected variables to be in the format of <name> <value>."
                        }
                    }
                }
            } else {
                if value.is_some() {
                    bail! {
                        labels = vec![entry.span().with_label("here")],
                        "Default variable value may only be set once"
                    }
                }

                let s = entry.value();
                let s = if let Some(s) = s.as_string() {
                    s.to_string()
                } else if let Some(v) = s.as_integer() {
                    v.to_string()
                } else if let Some(v) = s.as_float() {
                    v.to_string()
                } else {
                    bail! {
                        labels = vec![entry.span().with_label("here")],
                        "Expected variable value to be a string or number."
                    }
                };
                value = Some(VariableValue::Str(s));
            }
        }

        let Some(value) = value else {
            bail! {
                labels = vec![node.span().with_label("here")],
                "Expected variable value to be one of `<value>`, `file=<path>`, or `env=<env-var>`."
            }
        };

        Ok((
            name,
            Variable {
                persist,
                default_value: value,
            },
        ))
    }

    fn get_string(&self) -> miette::Result<Cow<'_, str>> {
        match &self.default_value {
            VariableValue::Str(s) => Ok(Cow::Borrowed(s)),
            VariableValue::File(path) => std::fs::read_to_string(path)
                .map(|mut s| {
                    // effectively just `trim`, but it does so without needing to re-allocate
                    while s.ends_with(char::is_whitespace) {
                        s.pop();
                    }
                    while s.starts_with(char::is_whitespace) {
                        s.remove(0);
                    }
                    Cow::Owned(s)
                })
                .into_diagnostic()
                .with_context(|| format!("Reading variable from path {:?}", path)),
            VariableValue::Env { var, span } => match std::env::var(var) {
                Ok(v) => Ok(Cow::Owned(v)),
                Err(e) => {
                    bail! {
                        labels = vec![span.with_label("here")],
                        "Env Error: {}", e,
                    }
                }
            },
        }
    }

    fn load<'a>(&'a self, name: &str, state: &State) -> miette::Result<Interpolated<'a>> {
        // if this variable should be persisted
        if self.persist.persists()
            // and it has been persisted
            && let Some(persisted) = state.variables.get(name)
            // and it isn't expired
            && persisted.expires_at.is_none_or(|e| e > Utc::now())
        {
            return Ok(persisted.value.clone().into());
        }

        self.get_string().map(Into::into)
    }
}

pub type Variables<'a> = HashMap<String, Interpolated<'a>>;

#[derive(Debug, Clone)]
pub struct Config {
    doc: KdlDocument,
    variables: HashMap<String, Variable>,
}

impl Config {
    pub fn parse(doc: KdlDocument) -> miette::Result<Self> {
        Ok(Self {
            variables: Self::parse_variables(&doc)?,
            doc,
        })
    }

    fn parse_variables(doc: &KdlDocument) -> miette::Result<HashMap<String, Variable>> {
        let mut out = HashMap::new();
        let Some(vars) = doc.get("variables") else {
            return Ok(out);
        };

        out.reserve(vars.children().map(|d| d.nodes().len()).unwrap_or(0));

        for n in vars.iter_children() {
            let (name, var) = Variable::parse(n)?;
            out.insert(name, var);
        }

        Ok(out)
    }

    pub fn load_variables<'a>(
        &'a self,
        cli: &'a SubCmd,
        state: &State,
    ) -> miette::Result<Variables<'a>> {
        let mut out = HashMap::with_capacity(self.variables.len());
        for (name, var) in &self.variables {
            let val = if let Some(var) = cli.get_variable(name) {
                Interpolated::from(var)
            } else {
                var.load(name, state)?
            };
            out.insert(name.clone(), val);
        }
        Ok(out)
    }

    pub fn get_variable(&self, name: &'_ str) -> Option<&Variable> {
        self.variables.get(name)
    }

    pub fn get_query<'a>(&'a self, name: &str) -> miette::Result<Option<Query<'a>>> {
        let q = self
            .doc
            .get("queries")
            .context("Missing queries node")?
            .children()
            .context("Missing queries children")?
            .get(name);

        if let Some(q) = q {
            Query::from_node(q)
        } else {
            Ok(None)
        }
    }
}
