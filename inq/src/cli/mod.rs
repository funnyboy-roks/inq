use std::path::PathBuf;

use clap::{Parser, Subcommand};
use humantime::Duration;

pub mod eval;
pub mod route;
pub mod variable;

#[derive(Debug, Parser)]
pub struct RouteCommand {
    /// Print the raw body of the response
    #[clap(short, long)]
    pub raw: bool,
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
