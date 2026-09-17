use std::{borrow::Cow, collections::HashMap, rc::Rc, time::Duration};

use fuzzt::processors::{LowerAlphaNumStringProcessor, StringProcessor};
use inq_lang::{Attribute, Ident, Item, Parser, Route, eval::Engine};
use miette::{Context, IntoDiagnostic, SourceSpan};
use reqwest::{Method, blocking::Client};

use crate::script;

#[derive(Debug, Clone)]
#[expect(unused)] // while refactoring
pub struct OldQuery<'a> {
    pub(crate) _name: &'a str,
    pub(crate) name_span: SourceSpan,
    pub(crate) method: Method,
    pub(crate) url: String,
    pub(crate) body: Option<()>,
    pub(crate) headers: HashMap<Cow<'a, str>, String>,
    pub post_script: Option<()>,
}

#[derive(Debug, Clone)]
#[expect(unused)]
pub struct OldClientConfig {
    headers: HashMap<String, String>,
    redirect: Option<usize>,
    timeout: Option<Option<Duration>>,
    connect_timeout: Option<Option<Duration>>,
    interface: Option<String>,
}

impl OldClientConfig {
    fn default_headers() -> HashMap<String, String> {
        HashMap::from_iter([(
            "user-agent".into(),
            concat!("inq/", env!("CARGO_PKG_VERSION")).into(),
        )])
    }

    // TODO(refactor) CLIENT:
    //
    // // from https://docs.rs/reqwest/latest/src/reqwest/blocking/client.rs.html#720-722
    // #[cfg(any(
    //     target_os = "android",
    //     target_os = "fuchsia",
    //     target_os = "illumos",
    //     target_os = "ios",
    //     target_os = "linux",
    //     target_os = "macos",
    //     target_os = "solaris",
    //     target_os = "tvos",
    //     target_os = "visionos",
    //     target_os = "watchos",
    // ))]
    // if let Some(interface) = &self.interface {
    //     builder = builder.interface(interface);
    // }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) routes: Vec<Route>,
    pub(crate) persisted_vars: Vec<Ident>,
    pub(crate) engine: Rc<Engine>,
}

impl Config {
    pub fn load(content: &str) -> miette::Result<Self> {
        let mut this = Self {
            routes: Default::default(),
            persisted_vars: Default::default(),
            engine: script::base_engine(),
        };

        this.read_items(content)?;

        Ok(this)
    }

    fn read_items(&mut self, content: &str) -> miette::Result<()> {
        let mut parser = Parser::new(content)?;

        while let Some(item) = parser.take_item()? {
            match item {
                Item::Variable(v) => {
                    if v.attributes.contains(&Attribute::Persist) {
                        self.persisted_vars.push(v.name.clone());
                    }
                    self.engine.global().add_variable(v);
                }
                Item::Route(r) => self.routes.push(r),
            }
        }

        Ok(())
    }

    /// Get a route that matches the provided name
    pub(crate) fn get_route(&self, name: &str) -> Option<&Route> {
        self.routes.iter().find(|r| r.name == name)
    }

    /// Get a route that matches the provided name
    pub(crate) fn expect_route(&self, name: &str) -> miette::Result<&Route> {
        self.routes
            .iter()
            .find(|r| r.name == name)
            .ok_or_else(|| self.closest_route_error(name))
    }

    /// Get the closest route to the provided name
    fn closest_route_error(&self, name: &str) -> miette::Error {
        let closest = self
            .routes
            .iter()
            .map(|r| {
                (
                    fuzzt::algorithms::normalized_levenshtein(
                        &LowerAlphaNumStringProcessor.process(name),
                        &LowerAlphaNumStringProcessor.process(&r.name.to_string()),
                    ),
                    r,
                )
            })
            .max_by(|l, r| l.0.total_cmp(&r.0));

        let Some(closest) = closest else {
            return miette::miette!("No routes defined");
        };

        let (lev, closest) = closest;

        if lev > 0.5 {
            miette::miette! {
                labels = vec![closest.name.span().with_label("Similarly named route defined here")],
                help = "Another route with a similar name exists",
                "Route '{}' not found", name
            }
        } else {
            miette::miette! {
                "Route '{}' not found", name
            }
        }
    }

    pub fn client(&self) -> miette::Result<Client> {
        Client::builder()
            .build()
            .into_diagnostic()
            .context("Building client")
    }
}
