use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

impl Variable {
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

#[derive(Debug, Parser)]
pub struct QueryCommand {
    /// Print the raw body of the response
    #[clap(short, long)]
    pub raw: bool,
    pub query: String,
    #[clap(short, long, value_parser = Variable::parse)]
    var: Vec<Variable>,
}

impl QueryCommand {
    pub fn get_variable(&self, name: &'_ str) -> Option<&str> {
        self.var.iter().find(|v| v.name == name).map(|s| &*s.value)
    }
}

#[derive(Debug, Subcommand)]
pub enum SubCmd {
    /// Execute a query
    #[clap(alias = "q")]
    Query(QueryCommand),
}

impl SubCmd {
    pub fn get_variable(&self, name: &'_ str) -> Option<&str> {
        match self {
            SubCmd::Query(query_command) => query_command.get_variable(name),
        }
    }
}

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(short, long, default_value = "inq.kdl")]
    pub config: PathBuf,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}
