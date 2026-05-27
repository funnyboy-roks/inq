use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use chrono::Utc;
use kdl::{KdlDocument, KdlNode, KdlValue};
use miette::{Context, IntoDiagnostic, SourceSpan, bail};
use reqwest::{
    Method,
    blocking::{Client, Request},
    redirect,
};
use rhai::{AST, Engine};
use serde_json::Value as JsonValue;

use crate::{
    cli::SubCmd,
    parse::{
        expect_entry, get_entry_string, get_entry_string_named, get_one_of, unique_entry,
        unique_node,
    },
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
    pub(crate) _name: &'a str,
    pub(crate) name_span: SourceSpan,
    pub(crate) method: Method,
    pub(crate) url: Interpolated<'a>,
    pub(crate) body: Option<Body<'a>>,
    pub(crate) headers: HashMap<&'a str, Interpolated<'a>>,
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
            name_span: node.name().span(),
            method,
            url: url.into(),
            body: None,
            headers: Default::default(),
            post_script: None,
        })
    }

    fn parse_headers(
        node: &'a KdlNode,
        default_headers: &'a HashMap<String, Interpolated<'static>>,
    ) -> miette::Result<HashMap<&'a str, Interpolated<'a>>> {
        let mut headers: HashMap<&str, Interpolated<'_>> = default_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        for n in node.iter_children() {
            let name = n.name().value();

            if name.starts_with('~') {
                headers.remove(&name['~'.len_utf8()..]);
                if !n.entries().is_empty() {
                    bail! {
                        labels = vec![n.span().with_label("This node")],
                        "Negated headers may not have a value."
                    }
                }
                continue;
            }

            let (_, value) = get_entry_string_named(n, 0, true, "header value")?
                .context("Expected header value to be a string.")?;

            headers.insert(name, value.into());
        }
        Ok(headers)
    }

    pub fn new(node: &'a KdlNode, client_config: &'a ClientConfig) -> miette::Result<Self> {
        let this = Self::parse_heading(node)?;

        let Some(children) = node.children() else {
            // if no children, we're done parsing
            return Ok(this);
        };

        let body = if let Some(body_node) = unique_node(children, "body")? {
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

        let headers = unique_node(children, "headers")?
            .map(|n| Self::parse_headers(n, &client_config.headers))
            .transpose()?
            .unwrap_or_else(|| {
                client_config
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.clone()))
                    .collect()
            });

        let post_script = if let Some(node) = unique_node(children, "post-script")? {
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
        let Some(entry) = unique_entry(node, "persist")? else {
            return Ok(Persist::Never);
        };

        match entry.value() {
            KdlValue::String(s) => match humantime::parse_duration(s) {
                Ok(d) => Ok(Persist::Duration(d)),
                Err(e) => bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Unable to parse duration: {}", e,
                },
            },
            KdlValue::Bool(true) => Ok(Persist::Forever),
            _ => bail! {
                labels = vec![entry.span().with_label("here")],
                "Persist must either be #true or a duration like \"1 hour\""
            },
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

#[derive(Debug, Clone, Default)]
pub struct ClientConfig {
    headers: HashMap<String, Interpolated<'static>>,
    redirect: Option<usize>,
    timeout: Option<Option<Duration>>,
    connect_timeout: Option<Option<Duration>>,
    interface: Option<String>,
}
impl ClientConfig {
    fn parse_headers(node: &KdlNode) -> miette::Result<HashMap<String, Interpolated<'static>>> {
        let mut headers = HashMap::new();
        for n in node.iter_children() {
            let (_, value) = get_entry_string_named(n, 0, true, "header value")?
                .context("Expected header value to be a string.")?;

            headers.insert(
                n.name().value().into(),
                Interpolated::from(value).to_owned(),
            );
        }
        Ok(headers)
    }

    fn new(children: &KdlDocument) -> miette::Result<Self> {
        let headers = unique_node(children, "headers")?
            .map(Self::parse_headers)
            .transpose()?
            .unwrap_or_default();

        let redirect = if let Some(redirect) = unique_node(children, "redirect")? {
            let entry = expect_entry(
                redirect,
                "limit",
                "Redirect must have a limit specified: redirect limit=5",
            )?;

            let Some(limit) = entry.value().as_integer() else {
                bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Limit must be an integer"
                }
            };

            if !(0..=64).contains(&limit) {
                bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Limit must be in the range [0, 64]"
                }
            }

            Some(limit as usize)
        } else {
            None
        };

        let timeout = if let Some(timeout) = unique_node(children, "timeout")? {
            let entry = expect_entry(
                timeout,
                0,
                "Timeout expects a value: timeout <duration> OR timeout #false",
            )?;

            let duration = match entry.value() {
                KdlValue::String(s) => Some(humantime::parse_duration(s).map_err(|e| {
                    miette::miette! {
                        labels = vec![entry.span().with_label("here")],
                        "{}", e
                    }
                })?),
                KdlValue::Bool(false) => None,
                _ => bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Timeout must be a duration or #false."
                },
            };
            Some(duration)
        } else {
            None
        };

        let connect_timeout = if let Some(connect_timeout) =
            unique_node(children, "connect-timeout")?
        {
            let entry = expect_entry(
                connect_timeout,
                0,
                "Connect timeout expects a value: connect-timeout <duration> OR connect-timeout #false",
            )?;

            let duration = match entry.value() {
                KdlValue::String(s) => Some(humantime::parse_duration(s).map_err(|e| {
                    miette::miette! {
                        labels = vec![entry.span().with_label("here")],
                        "{}", e
                    }
                })?),
                KdlValue::Bool(false) => None,
                _ => bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Connect timeout must be a duration or #false."
                },
            };
            Some(duration)
        } else {
            None
        };

        let interface = if let Some(iface) = unique_node(children, "interface")? {
            // from https://docs.rs/reqwest/latest/src/reqwest/blocking/client.rs.html#720-722
            #[cfg(not(any(
                target_os = "android",
                target_os = "fuchsia",
                target_os = "illumos",
                target_os = "ios",
                target_os = "linux",
                target_os = "macos",
                target_os = "solaris",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            )))]
            {
                crate::warn!("`interface` property ignored on this platform.");
            }
            let entry = expect_entry(iface, 0, "Interface expects a value: interface \"eth0\"")?;

            let Some(iface) = entry.value().as_string() else {
                bail! {
                    labels = vec![entry.span().with_label("here")],
                    "Interface name must be a string"
                }
            };

            Some(iface.into())
        } else {
            None
        };

        Ok(ClientConfig {
            headers,
            redirect,
            timeout,
            connect_timeout,
            interface,
        })
    }

    fn make_client(&self, _vars: &HashMap<String, Interpolated<'_>>) -> miette::Result<Client> {
        let mut builder = Client::builder();

        if let Some(limit) = self.redirect {
            if limit == 0 {
                builder = builder.redirect(redirect::Policy::none());
            } else {
                builder = builder.redirect(redirect::Policy::limited(limit));
            }
        }

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }

        // from https://docs.rs/reqwest/latest/src/reqwest/blocking/client.rs.html#720-722
        #[cfg(any(
            target_os = "android",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "ios",
            target_os = "linux",
            target_os = "macos",
            target_os = "solaris",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        ))]
        if let Some(interface) = &self.interface {
            builder = builder.interface(interface);
        }

        let client = builder
            .build()
            .into_diagnostic()
            .context("Building http client")?;

        Ok(client)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    doc: KdlDocument,
    variables: HashMap<String, Variable>,
    client: ClientConfig,
}

impl Config {
    pub fn parse(doc: KdlDocument) -> miette::Result<Self> {
        Ok(Self {
            variables: Self::parse_variables(&doc)?,
            client: Self::parse_client(&doc)?,
            doc,
        })
    }

    fn parse_variables(doc: &KdlDocument) -> miette::Result<HashMap<String, Variable>> {
        let mut out = HashMap::new();
        let Some(vars) = unique_node(doc, "variables")? else {
            return Ok(out);
        };

        out.reserve(vars.children().map(|d| d.nodes().len()).unwrap_or(0));

        for n in vars.iter_children() {
            let (name, var) = Variable::parse(n)?;
            out.insert(name, var);
        }

        Ok(out)
    }

    fn parse_client(doc: &KdlDocument) -> miette::Result<ClientConfig> {
        let Some(client) = unique_node(doc, "client")? else {
            return Ok(Default::default());
        };

        let Some(children) = client.children() else {
            bail! {
                labels = vec![client.span().with_label("here")],
                "Client block must have children."
            }
        };

        ClientConfig::new(children)
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

    pub(crate) fn make_client(&self, vars: &Variables<'_>) -> miette::Result<Client> {
        self.client.make_client(vars)
    }

    pub fn get_query<'a>(&'a self, name: &str) -> miette::Result<Option<Query<'a>>> {
        unique_node(&self.doc, "queries")?
            .context("Missing queries node")?
            .children()
            .context("Missing queries children")?
            .get(name)
            .map(|q| Query::new(q, &self.client))
            .transpose()
    }

    pub fn queries<'a>(
        &'a self,
    ) -> miette::Result<impl Iterator<Item = (&'a str, miette::Result<Query<'a>>)>> {
        Ok(unique_node(&self.doc, "queries")?
            .context("Missing queries node")?
            .children()
            .context("Missing queries children")?
            .nodes()
            .iter()
            .map(|q| (q.name().value(), Query::new(q, &self.client))))
    }
}
