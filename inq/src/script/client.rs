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
use reqwest::{blocking::Client, redirect::Policy};

use crate::script::duration::DurationValue;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    redirect: Option<usize>,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    interface: Option<IStr>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            redirect: None,
            timeout: Some(Duration::from_secs(3)),
            connect_timeout: None,
            interface: None,
        }
    }
}

impl From<ClientConfig> for Client {
    fn from(value: ClientConfig) -> Self {
        let mut builder = Client::builder();

        builder = builder.redirect(match value.redirect {
            Some(n) => Policy::limited(n),
            None => Policy::none(),
        });

        builder = builder.timeout(value.timeout);
        builder = builder.connect_timeout(value.connect_timeout);

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
        if let Some(iface) = value.interface {
            builder = builder.interface(&iface);
        }

        builder.build().unwrap()
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

    fn snapshot(&self) -> Rc<RefCell<dyn Value>> {
        Rc::new(RefCell::new(self.clone()))
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("Client").finish_non_exhaustive()
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get_set(
            "redirects",
            |_, this| this.redirect.map(|n| n as Int),
            |ctx, this, value: ValueRef| {
                this.redirect = if let Some(n) = value.downcast::<Int>() {
                    if n <= 0 {
                        return Err(ctx.error("Redirects must be > 0"));
                    } else {
                        Some(n as usize)
                    }
                } else if value.is::<Null>() {
                    None
                } else {
                    return Err(ctx.error("Redirects must be an integer or null"));
                };
                Ok(())
            },
        );

        registry.register_field_get_set(
            "timeout",
            |_, this| this.timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
                this.timeout = if let Some(d) = value.downcast::<DurationValue>() {
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
            |_, this| this.timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
                this.connect_timeout = if let Some(d) = value.downcast::<DurationValue>() {
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
            "interface",
            |_, this| this.timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
                this.interface = Some(value.expect_downcast::<IStr>(ctx.span())?);
                Ok(())
            },
        );
    }
}
