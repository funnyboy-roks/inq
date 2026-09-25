use std::{collections::HashMap, rc::Rc, str::FromStr, time::Duration};

use fuzzt::processors::{LowerAlphaNumStringProcessor, StringProcessor};
use inq_lang::{
    Attribute, IStr, Ident, Item, Parser, Route, Span,
    eval::{Engine, EvalError, value::ValueRef},
};
use reqwest::Url;

use crate::script::{self, url::UrlValue};

#[derive(Debug, Clone)]
#[expect(unused)]
pub struct OldClientConfig {
    headers: HashMap<String, String>,
    redirect: Option<usize>,
    timeout: Option<Option<Duration>>,
    connect_timeout: Option<Option<Duration>>,
    interface: Option<String>,
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

        this.parse_items(content)?;

        Ok(this)
    }

    fn parse_items(&mut self, content: &str) -> miette::Result<()> {
        let mut parser = Parser::new(content)?;

        while let Some(item) = parser.take_item()? {
            match item {
                Item::Variable(v) => {
                    if v.attributes.contains(&Attribute::Persist) {
                        self.persisted_vars.push(v.name.clone());
                    }
                    if v.name == "BASE_URL" {
                        self.engine.global().add_mapped_variable(
                            v,
                            |span: Span, value: ValueRef| {
                                let s = value.expect_downcast::<IStr>(span)?;
                                let url = Url::from_str(&s).map_err(|e| {
                                    EvalError::custom(span, format!("Unable to parse url: {}", e))
                                })?;

                                Ok(ValueRef::from(UrlValue(url)))
                            },
                        );
                    } else {
                        self.engine.global().add_variable(v);
                    }
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
        self.get_route(name)
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
}
