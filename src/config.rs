use std::{borrow::Cow, collections::HashMap, str::FromStr};

use kdl::{KdlDocument, KdlNode};
use miette::{Context, IntoDiagnostic, SourceSpan, bail};
use reqwest::{
    Method,
    blocking::{Client, Request},
};
use serde_json::Value as JsonValue;

use crate::util::{Interpolated, WithLabel};

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
            let body = if let Some(text) = body_node.entry("text")
                && body_node.len() == 1
            {
                let text = text
                    .value()
                    .as_string()
                    .context("Expected body.text to be a string.")?;
                Body::Text(text.into())
            } else if let Some(json) = body_node.entry("json")
                && body_node.len() == 1
            {
                Body::Json {
                    json: json
                        .value()
                        .as_string()
                        .context("Expected body.json to be a string.")?
                        .into(),
                    span: json.span(),
                }
            } else {
                return Err(miette::miette! {
                    labels = vec![body_node.span().with_label("here")],
                    "Malformed `body` node",
                });
            };
            Some(body)
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

    pub(crate) fn to_request<F>(
        &self,
        client: &Client,
        mut variable_getter: F,
    ) -> miette::Result<(Request, Option<PopulatedBody<'a>>)>
    where
        F: FnMut(&str) -> miette::Result<Option<String>>,
    {
        let url = self
            .url
            .interpolate(&mut variable_getter)
            .context("Interpolating URL")?;

        let mut builder = client.request(self.method.clone(), &*url);

        let populated_body = if let Some(body) = &self.body {
            match body {
                Body::Text(t) => {
                    let s = t.interpolate(&mut variable_getter)?;
                    builder = builder.body(s.clone().into_owned());
                    Some(PopulatedBody::Text(s))
                }
                Body::Json { json, span } => {
                    let interpolated = &json.interpolate(&mut variable_getter)?;
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

        for (&k, &v) in &self.headers {
            builder = builder.header(k, &*v.interpolate(&mut variable_getter)?);
        }

        Ok((builder.build().into_diagnostic()?, populated_body))
    }
}

#[derive(Debug, Clone)]
enum Variable {
    Str(String),
    Env { var: String, span: SourceSpan },
}

impl Variable {
    fn get_string(&self) -> miette::Result<Cow<'_, str>> {
        match self {
            Variable::Str(s) => Ok(Cow::Borrowed(s)),
            Variable::Env { var, span } => match std::env::var(var) {
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
}

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

        for n in vars.iter_children() {
            let name = n.name().value().to_string();
            match n.entries() {
                [] if let Some(children) = n.children() => {
                    if let Some(env) = children.get("env")
                        && children.nodes().len() == 1
                        && env.len() <= 1
                    {
                        let (var, span) = if let Some(val) = env.get(0) {
                            if let Some(s) = val.as_string() {
                                (s.to_string(), env.span())
                            } else {
                                bail! {
                                    labels = vec![env.span().with_label("here")],
                                    "Expected string",
                                }
                            }
                        } else {
                            (name.clone(), n.name().span())
                        };

                        out.insert(name, Variable::Env { var, span });
                    } else {
                        bail! {
                            labels = vec![n.span().with_label("here")],
                            "Invalid variable structure",
                        }
                    }
                }
                [entry] => {
                    if entry.name().is_some() {
                        bail! {
                            labels = vec![entry.span().with_label("here")],
                            "Expected variables to be in the format of <name> <value>."
                        }
                    }

                    let value = entry.value();
                    let value = if let Some(s) = value.as_string() {
                        s.to_string()
                    } else if let Some(v) = value.as_integer() {
                        v.to_string()
                    } else if let Some(v) = value.as_float() {
                        v.to_string()
                    } else {
                        bail! {
                            labels = vec![entry.span().with_label("here")],
                            "Expected variable value to be a string or number."
                        }
                    };
                    out.insert(name, Variable::Str(value));
                }
                _ => bail! {
                    labels = vec![n.span().with_label("here")],
                    "Expected variables to be in the format of <name> <value>."
                },
            }
        }

        Ok(out)
    }

    pub fn get_variable<'a>(&'a self, name: &str) -> miette::Result<Option<Cow<'a, str>>> {
        self.variables
            .get(name)
            .map(Variable::get_string)
            .transpose()
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
