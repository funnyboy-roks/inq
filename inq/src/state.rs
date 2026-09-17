use std::{
    collections::HashMap,
    io::BufWriter,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use inq_lang::IStr;
use miette::Context;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::config::Config;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PersistedVariable {
    value: IStr,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    variables: HashMap<IStr, PersistedVariable>,
}

impl State {
    fn data_dir(config_path: &Path) -> miette::Result<PathBuf> {
        let parent = config_path.parent().context("Invalid config path")?;
        let data_dir = parent.join(".inq");
        Ok(data_dir)
    }

    fn load_data_file_or<T: DeserializeOwned>(
        data_dir: &Path,
        name: &str,
        or: impl FnOnce() -> T,
    ) -> miette::Result<T> {
        let vars = data_dir.join("variables.json");
        if vars.exists() {
            let f = std::io::BufReader::new(
                std::fs::File::open(vars)
                    .map_err(|e| miette::miette!("Error reading .inq/{}: {}", name, e))?,
            );

            serde_json::from_reader(f)
                .map_err(|e| miette::miette!("Error reading .inq/{}: {}", name, e))
        } else {
            Ok(or())
        }
    }

    pub fn load(config_path: impl AsRef<Path>) -> miette::Result<Self> {
        let data_dir = Self::data_dir(config_path.as_ref())?;

        Ok(Self {
            variables: Self::load_data_file_or(&data_dir, "variables.json", Default::default)?,
        })
    }

    pub fn update_variables(&mut self, config: &Config) -> miette::Result<()> {
        for var in &config.persisted_vars {
            let v = config
                .engine
                .global()
                .get_variable(&var.as_istr())?
                .expect("Only added if declared");
            if let Some(s) = v.downcast::<IStr>() {
                self.variables.insert(
                    var.as_istr(),
                    PersistedVariable {
                        value: s,
                        expires_at: None,
                    },
                );
            }
        }
        Ok(())
    }

    pub fn save(&self, config_path: impl AsRef<Path>) -> miette::Result<()> {
        let data_dir = Self::data_dir(config_path.as_ref())?;

        let dir_existed = data_dir.exists();

        std::fs::create_dir_all(&data_dir)
            .map_err(|e| miette::miette!("Error creating {:?}: {}", data_dir, e))?;

        if !dir_existed {
            std::fs::write(data_dir.join(".gitignore"), "*\n")
                .map_err(|e| miette::miette!("Error writing .inq/.gitignore: {}", e))?;
        }

        let variables = data_dir.join("variables.json");
        let variables = std::fs::File::create(variables)
            .map_err(|e| miette::miette!("Error creating .inq/variables.json: {}", e))?;
        let variables = BufWriter::new(variables);
        serde_json::to_writer_pretty(variables, &self.variables)
            .map_err(|e| miette::miette!("Error writing .inq/variables.json: {}", e))?;

        Ok(())
    }
}
