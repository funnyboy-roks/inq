use std::collections::HashMap;

use kdl::{KdlDocument, KdlNode};
use miette::{Context, IntoDiagnostic, bail};
use reqwest::{
    Method,
    blocking::{Client, RequestBuilder},
};

use crate::util::Interpolated;

#[derive(Debug, Clone)]
pub enum Body<'a> {
    Text(Interpolated<'a>),
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
            && let Some(text) = body_node.entry("text")
        {
            let text = text
                .value()
                .as_string()
                .context("Expected body.text to be a string.")?;
            Some(Body::Text(text.into()))
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

    pub(crate) fn to_request<F>(&self, mut variable_getter: F) -> miette::Result<RequestBuilder>
    where
        F: FnMut(&'a str) -> Option<&'a str>,
    {
        let url = self
            .url
            .interpolate(&mut variable_getter)
            .context("Interpolating URL")?;
        eprintln!("Sending request to {}", url);
        let mut builder = Client::new().request(self.method.clone(), &*url);

        if let Some(body) = &self.body {
            builder = match body {
                Body::Text(t) => builder.body(t.interpolate(&mut variable_getter)?.to_string()),
            };
        }

        for (&k, &v) in &self.headers {
            builder = builder.header(k, &*v.interpolate(&mut variable_getter)?);
        }

        Ok(builder)
    }
}

pub struct Config {
    doc: KdlDocument,
    variables: HashMap<String, String>,
}

impl Config {
    pub fn parse(doc: KdlDocument) -> miette::Result<Self> {
        Ok(Self {
            variables: Self::parse_variables(&doc)?,
            doc,
        })
    }

    fn parse_variables(doc: &KdlDocument) -> miette::Result<HashMap<String, String>> {
        let mut out = HashMap::new();
        let Some(vars) = doc.get("variables") else {
            return Ok(out);
        };

        for n in vars.iter_children() {
            let name = n.name().value().to_string();
            let value = n
                .entry(0)
                .context("Expected <name> <value> variables.")?
                .value();
            let value = if let Some(s) = value.as_string() {
                s.to_string()
            } else if let Some(v) = value.as_integer() {
                v.to_string()
            } else if let Some(v) = value.as_float() {
                v.to_string()
            } else {
                bail!("Expected variable value to be a string or number.");
            };
            out.insert(name, value);
        }

        Ok(out)
    }

    pub fn get_variable<'a>(&'a self, name: &str) -> Option<&'a str> {
        self.variables.get(name).map(AsRef::as_ref)
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
