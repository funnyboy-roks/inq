use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use humantime::Duration;

pub mod eval;
pub mod route;
pub mod variable;

#[derive(Debug, Args, Default)]
pub struct CliClientConfig {
    /// The number of redirects allowed by the client.  If not specified, then no limit is set
    #[clap(long)]
    pub redirects: Option<usize>,
    /// The maximum time to wait for a response after sending a request.  Default: 30s
    #[clap(long)]
    pub timeout: Option<humantime::Duration>,
    /// The maximum time to wait to establish a connection with the remote server.  Default: unlimited
    #[clap(long)]
    pub connect_timeout: Option<humantime::Duration>,
    /// The interface upon which to connect to the the remote server.
    ///
    /// NOTE: This flag is not supported on Windows
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
    #[clap(long)]
    pub interface: Option<String>,
}

#[derive(Debug, Parser)]
pub struct RouteCommand {
    /// Print the raw body of the response
    #[clap(short, long)]
    pub raw: bool,
    #[clap(flatten)]
    pub client: CliClientConfig,
    /// The route to run.  If not passed, all routes will be listed
    pub route: Option<String>,
    /// Arguments to pass to the route
    ///
    /// Optional arguments (those with `= <value>`) may be omitted
    pub args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum VariableSubCmd {
    /// Set a specific variable
    ///
    /// At least one of `VALUE` or `--expires` must be set.  If
    /// `--expires` is set without a value, then the expiration date is updated without changing the
    /// value and vice versa.
    Set {
        #[clap(short, long)]
        expires: Option<Duration>,
        variable: String,
        value: Option<String>,
    },
    /// Get the value of a persisted variable
    Get { variable: String },
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
    /// Execute a query
    #[clap(alias = "r", alias = "q", alias = "query")]
    Route(RouteCommand),
    /// Manipulate persisted variables
    #[clap(alias = "var")]
    Variable(VariableCommand),
    /// Evaluate the contents of a file as if it were a script
    #[clap()]
    Eval(EvalCommand),
}

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(short, long, default_value = "main.inq")]
    pub config: PathBuf,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}
