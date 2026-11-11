use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use humantime::Duration;

pub mod eval;
pub mod route;
pub mod variable;

#[derive(Debug, Clone)]
pub struct NamedRouteArg {
    pub name: String,
    pub value: String,
}

impl NamedRouteArg {
    pub fn parse(s: &str) -> Result<Self, clap::error::Error> {
        if let Some((name, value)) = s.split_once('=') {
            Ok(Self {
                name: name.into(),
                value: value.into(),
            })
        } else {
            Err(clap::error::Error::raw(
                clap::error::ErrorKind::InvalidValue,
                "Expected KEY=VALUE",
            ))
        }
    }
}

#[derive(Debug, Args, Default)]
pub struct CliClientConfig {
    /// The number of redirects allowed by the client.  If not specified, then no limit is set
    #[clap(long)]
    pub max_redirects: Option<u32>,
    /// The maximum time to wait for a response after sending a request.  Default: 30s
    #[clap(long)]
    pub timeout: Option<humantime::Duration>,
    /// The maximum time to wait to establish a connection with the remote server.  Default: unlimited
    #[clap(long)]
    pub connect_timeout: Option<humantime::Duration>,
    /// The interface upon which to connect to the the remote server.
    ///
    /// NOTE: This flag is ignored on Windows
    #[clap(long)]
    pub interface: Option<String>,
    /// Whether inq should allow requests to URLs with invalid certs (i.e., self-hosted certs)
    #[clap(short, long)]
    pub insecure: bool,
}

#[derive(Debug, Parser)]
pub struct RouteCommand {
    /// Print the raw body of the response, rather than attempting to format it
    #[clap(short, long)]
    pub raw: bool,
    #[clap(flatten)]
    pub client: CliClientConfig,
    /// The route to run.  If not passed, all routes will be listed
    pub route: Option<String>,
    /// Route arguments that are identified by their name
    ///
    /// Should be used in the format of `--arg <name>=<value>`.  This flag may be specified multiple times
    #[clap(short = 'a', long = "arg", value_parser = NamedRouteArg::parse)]
    pub named_args: Vec<NamedRouteArg>,
    /// Route arguments that are identified by their position
    ///
    /// Arguments defined with `= <value>` may be ommited.  All other arguments must be assigned a
    /// value.
    ///
    /// See Also: `--arg`
    pub args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum VariableSubCmd {
    /// Set the value of a specific variable
    ///
    /// At least one of `VALUE` or `--expires` must be set.  If `--expires` is set without a value,
    /// then the expiration date is updated without changing the value and vice versa.
    Set {
        /// Time until `VARIABLE` should expire
        ///
        /// Example:
        ///     inq var set my_var --expires 5m
        #[clap(short, long, verbatim_doc_comment)]
        expires: Option<Duration>,
        /// The variable to update
        variable: String,
        /// The new value of `VARIABLE`
        ///
        /// Example:
        ///     inq var set my_var hello
        #[clap(verbatim_doc_comment)]
        value: Option<String>,
    },
    /// Get the value of a persisted variable
    Get {
        /// The variable to get
        variable: String,
    },
    /// List all persisted variables
    List,
}

#[derive(Debug, Parser)]
pub struct EvalCommand {
    /// File to evaluate
    pub file: PathBuf,
}

#[derive(Debug, Parser)]
pub struct VariableCommand {
    #[clap(subcommand)]
    pub command: VariableSubCmd,
}

#[derive(Debug, Subcommand)]
pub enum SubCmd {
    /// Execute or view routes
    ///
    /// When no route is specified (`inq route`), all available routes will be listed.  
    /// Optional route arguments will show with a `*`.
    ///
    /// Specifying a route will run that route with whatever arguments are provided.
    /// If the route has default arguments, those arguments may be ommitted.
    /// Additionally, arguments may be specified in key-value form, see `--arg` for more information.
    #[clap(alias = "r", alias = "routes", alias = "q", alias = "query")]
    Route(RouteCommand),
    /// Manipulate persisted variables
    ///
    /// Commands to get or set the variables persisted in `.inq/variables.json`
    #[clap(alias = "var")]
    Variable(VariableCommand),
    /// Evaluate the contents of a file as if it were a script
    ///
    /// This exists largely for debugging purposes, but it is also useful to explore the built-in
    /// features of inq!
    #[clap()]
    Eval(EvalCommand),
}

/// Commandline interface for `inq`
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    help_template = "{before-help}{name} {version}\n{author-with-newline}\n{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}{after-help}"
)]
pub struct Cli {
    #[clap(short, long, default_value = "api.inq")]
    pub config: PathBuf,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}
