use std::path::PathBuf;

use clap::Parser;

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
pub struct Cli {
    #[clap(short, long, value_parser = Variable::parse)]
    pub var: Vec<Variable>,
    #[clap(short, long, default_value = "inq.kdl")]
    pub config: PathBuf,
    /// Print the raw body of the response
    #[clap(short, long)]
    pub raw: bool,
    pub query: String,
}

impl Cli {
    pub fn get_variable(&self, name: &'_ str) -> Option<&str> {
        self.var.iter().find(|v| v.name == name).map(|s| &*s.value)
    }
}
