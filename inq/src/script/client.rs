use std::{cell::RefCell, rc::Rc, time::Duration};

use inq_lang::{
    IStr,
    eval::{
        registry::Registry,
        value::{
            Value, ValueRef,
            native::{Int, Null},
        },
    },
};
use ureq::{Agent, tls::TlsConfig};

use crate::{cli::CliClientConfig, debug_fmt, script::duration::DurationValue};

#[derive(Debug, Clone, derive_more::Deref, PartialEq, Eq)]
pub struct ClientConfig(pub RefCell<ClientConfigInner>);

impl From<ClientConfigInner> for ClientConfig {
    fn from(value: ClientConfigInner) -> Self {
        Self(RefCell::new(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfigInner {
    max_redirects: u32,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
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
    interface: Option<IStr>,
    allow_insecure: bool,
}

impl From<CliClientConfig> for ClientConfig {
    fn from(value: CliClientConfig) -> Self {
        ClientConfigInner {
            max_redirects: value.max_redirects.unwrap_or(0),
            timeout: Some(
                value
                    .timeout
                    .map(Into::into)
                    .unwrap_or(Duration::from_secs(30)),
            ),
            connect_timeout: value.connect_timeout.map(Into::into),
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
            interface: value.interface.map(Into::into),
            allow_insecure: value.insecure,
        }
        .into()
    }
}

impl From<ClientConfig> for Agent {
    fn from(value: ClientConfig) -> Self {
        let value = value.0.into_inner();

        let mut builder = Agent::config_builder().max_redirects_will_error(true);

        builder = builder.max_redirects(value.max_redirects);

        builder = builder.timeout_recv_response(value.timeout);
        builder = builder.timeout_connect(value.connect_timeout);

        if value.allow_insecure {
            builder = builder.tls_config(TlsConfig::builder().disable_verification(true).build());
        }

        builder.build().into()
    }
}

impl Value for ClientConfig {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Client".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str("Client");
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn eq(&self, other: ValueRef) -> bool {
        other.downcast_ref::<Self>().is_some_and(|o| o == self)
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_fmt! {
            into fmt as "Client",
            redirects       => self.borrow().max_redirects as Int,
            timeout         => self.borrow().timeout.map(DurationValue),
            connect_timeout => self.borrow().connect_timeout.map(DurationValue),
        }
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get_set(
            "redirects",
            |_, this| this.borrow().max_redirects as Int,
            |ctx, this, value: ValueRef| {
                let n = value.expect_downcast::<Int>(ctx.span())?;
                this.borrow_mut().max_redirects = n as u32;
                Ok(())
            },
        );

        registry.register_field_get_set(
            "timeout",
            |_, this| this.borrow().timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
                this.borrow_mut().timeout = if let Some(d) = value.downcast::<DurationValue>() {
                    Some(d.0)
                } else if value.is::<Null>() {
                    None
                } else {
                    return Err(ctx.error("Timeout must be a Duration or null"));
                };
                Ok(())
            },
        );

        registry.register_field_get_set(
            "connect_timeout",
            |_, this| this.borrow().timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
                this.borrow_mut().connect_timeout =
                    if let Some(d) = value.downcast::<DurationValue>() {
                        Some(d.0)
                    } else if value.is::<Null>() {
                        None
                    } else {
                        return Err(ctx.error("Timeout must be a Duration or null"));
                    };
                Ok(())
            },
        );

        registry.register_field_get_set(
            "allow_insecure",
            |_, this| this.borrow().timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
                this.borrow_mut().allow_insecure = value.expect_downcast::<bool>(ctx.span())?;
                Ok(())
            },
        );
    }
}
