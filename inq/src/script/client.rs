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

use crate::{cli::CliClientConfig, script::duration::DurationValue};

#[derive(Debug, Clone, derive_more::Deref)]
pub struct ClientConfig(pub RefCell<ClientConfigInner>);

impl From<ClientConfigInner> for ClientConfig {
    fn from(value: ClientConfigInner) -> Self {
        Self(RefCell::new(value))
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfigInner {
    redirect: Option<usize>,
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
}

impl From<CliClientConfig> for ClientConfig {
    fn from(value: CliClientConfig) -> Self {
        ClientConfigInner {
            redirect: value.redirects,
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
        }
        .into()
    }
}

impl From<ClientConfig> for Client {
    fn from(value: ClientConfig) -> Self {
        let value = value.0.into_inner();

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
        if let Some(iface) = &value.interface {
            builder = builder.interface(iface);
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

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("Client")
            .field(
                "redirects",
                &ValueRef::from(self.borrow().redirect.map(|n| n as Int)).debug(),
            )
            .field(
                "timeout",
                &ValueRef::from(self.borrow().timeout.map(DurationValue)).debug(),
            )
            .field(
                "connect_timeout",
                &ValueRef::from(self.borrow().connect_timeout.map(DurationValue)).debug(),
            )
            .field(
                "interface",
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
                &ValueRef::from(self.borrow().interface.as_ref().map(IStr::from)).debug(),
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
                &std::fmt::from_fn(|fmt| write!(fmt, "<disabled>")),
            )
            .finish()
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get_set(
            "redirects",
            |_, this| this.borrow().redirect.map(|n| n as Int),
            |ctx, this, value: ValueRef| {
                this.borrow_mut().redirect = if let Some(n) = value.downcast::<Int>() {
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
            "interface",
            |_, this| this.borrow().timeout.map(DurationValue),
            |ctx, this, value: ValueRef| {
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
                {
                    this.borrow_mut().interface = Some(value.expect_downcast::<IStr>(ctx.span())?);
                }
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
                crate::warn!("Interface is not supported on your operating system!");
                Ok(())
            },
        );
    }
}
