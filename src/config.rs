use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use chrono::Utc;
use kdl::{KdlDocument, KdlNode};
use miette::{Context, IntoDiagnostic, SourceSpan, bail};
use reqwest::{
    Method,
    blocking::{Client, Request},
};
use rhai::{AST, Engine};
use serde_json::Value as JsonValue;

use crate::{
    cli::SubCmd,
    parse::{get_entry_string, get_entry_string_named, get_one_of},
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
    pub post_script: Option<AST>,
}

impl<'a> Query<'a> {
    fn parse_heading(node: &'a KdlNode) -> miette::Result<Self> {
        let name = node.name().value();

        let method = Method::from_bytes(
            get_entry_string_named(node, 0, false, "Query method")?
                .context("Expected query method")?
                .1
                .as_bytes(),
        )
        .into_diagnostic()
        .context("Invalid query method")?;

        let (_, url) =
            get_entry_string_named(node, 1, false, "Query url")?.context("Expected query url")?;

        Ok(Self {
            _name: name,
            method,
            url: url.into(),
            body: None,
            headers: Default::default(),
            post_script: None,
        })
    }

    fn parse_headers(node: &KdlNode) -> miette::Result<HashMap<&str, Interpolated<'_>>> {
        let mut headers = HashMap::new();
        for n in node.iter_children() {
            let (_, value) = get_entry_string_named(n, 0, true, "header value")?
                .context("Expected header value to be a string.")?;

            headers.insert(n.name().value(), value.into());
        }
        Ok(headers)
    }

    pub fn from_node(node: &'a KdlNode) -> miette::Result<Self> {
        let this = Self::parse_heading(node)?;

        let Some(children) = node.children() else {
            // if no children, we're done parsing
            return Ok(this);
        };

        let body = if let Some(body_node) = children.get("body") {
            match get_one_of(body_node, "body", ["text", "json"])? {
                Some(("text", _, text)) => Some(Body::Text(text.into())),
                Some(("json", e, json)) => Some(Body::Json {
                    json: json.into(),
                    span: e.span(),
                }),
                v => {
                    dbg!(v);
                    return Err(miette::miette! {
                        labels = vec![body_node.span().with_label("here")],
                        "Malformed `body` node",
                    });
                }
            }
        } else {
            None
        };

        let headers = children
            .get("headers")
            .map(Self::parse_headers)
            .transpose()?
            .unwrap_or_default();

        let post_script = if let Some(node) = children.get("post-script") {
            if let Some((_, script)) = get_entry_string_named(node, 0, false, "post-script")? {
                Some(
                    Engine::new()
                        .compile(script)
                        .into_diagnostic()
                        .context("Compiling post-script")?,
                )
            } else {
                bail! {
                    labels = vec![node.span().with_label("here")],
                    "Malformed `post-script` node.  Expected `post-script <string>` ",
                }
            }
        } else {
            None
        };

        Ok(Self {
            body,
            headers,
            post_script,
            ..this
        })
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
    fn parse_value(node: &KdlNode) -> miette::Result<VariableValue> {
        // check for conflicting keys
        let require_1 = HashSet::<&str>::from_iter(["file", "env"]);
        let mut req1_count = 0;
        for e in node.entries() {
            if let Some(name) = e.name()
                && require_1.contains(name.value())
            {
                req1_count += 1;
            }
        }
        if req1_count > 1
            || (req1_count == 0 && node.entries().iter().find(|x| x.name().is_none()).is_none())
        {
            bail! {
                labels = vec![node.span().with_label("here")],
                help = "See the wiki for more information: https://codeberg.org/fbr/inq/wiki/Variables",
                "All variables must contain a default value."
            }
        }

        if let Some((_, path)) = get_entry_string(node, "file", false)? {
            let path = std::env::current_dir().into_diagnostic()?.join(&*path);

            return Ok(VariableValue::File(path));
        }

        if let Some((entry, var)) = get_entry_string(node, "env", false)? {
            return Ok(VariableValue::Env {
                var: var.into(),
                span: entry.span(),
            });
        }

        if let Some((_, s)) = get_entry_string(node, 0, true)? {
            return Ok(VariableValue::Str(s.into_owned()));
        }

        bail! {
            labels = vec![node.span().with_label("here")],
            help = "See the wiki for more information: https://codeberg.org/fbr/inq/wiki/Variables",
            "All variables must contain a default value."
        };
    }

    fn parse_persist(node: &KdlNode) -> miette::Result<Persist> {
        let Some(entry) = node.entry("persist") else {
            return Ok(Persist::Never);
        };

        if let Some(b) = entry.value().as_bool() {
            if !b {
                bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Persist must either be #true or a duration like \"1 hour\""
                }
            }

            Ok(Persist::Forever)
        } else if let Some(s) = entry.value().as_string() {
            match humantime::parse_duration(s) {
                Ok(d) => Ok(Persist::Duration(d)),
                Err(e) => bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Unable to parse duration: {}", e,
                },
            }
        } else {
            bail! {
                labels = vec![entry.span().with_label("here")],
                "Persist must either be #true or a duration like \"1 hour\""
            }
        }
    }

    fn parse(node: &KdlNode) -> miette::Result<(String, Self)> {
        let name = node.name().value().to_string();

        let value = Self::parse_value(node)?;
        let persist = Self::parse_persist(node)?;

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

    pub fn load_variables(
        &self,
        cli: &SubCmd,
        state: &State,
    ) -> miette::Result<Variables<'static>> {
        let mut out = HashMap::with_capacity(self.variables.len());
        for (name, var) in &self.variables {
            let val = if let Some(var) = cli.get_variable(name) {
                Interpolated::from(var).to_owned()
            } else {
                var.load(name, state)?.to_owned()
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
            Query::from_node(q).map(Some)
        } else {
            Ok(None)
        }
    }
}
