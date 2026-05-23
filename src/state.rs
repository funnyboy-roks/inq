use std::{
    collections::HashMap,
    io::BufWriter,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use miette::Context;
use rhai::{CustomType, Dynamic, EvalAltResult, Position, TypeBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, CustomType)]
pub struct PersistedVariable {
    pub value: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    /// contents of .inq/variables.json
    pub variables: HashMap<String, PersistedVariable>,
}

impl State {
    fn data_dir(config_path: &Path) -> miette::Result<PathBuf> {
        let parent = config_path.parent().context("Invalid config path")?;
        let data_dir = parent.join(".inq");
        Ok(data_dir)
    }

    pub fn load(config_path: impl AsRef<Path>) -> miette::Result<Self> {
        let data_dir = Self::data_dir(config_path.as_ref())?;

        let mut this = Self {
            variables: HashMap::new(),
        };

        let variables_json = data_dir.join("variables.json");
        if variables_json.exists() {
            let variables_json = std::fs::read_to_string(&variables_json)
                .map_err(|e| miette::miette!("Error opening {:?}: {}", variables_json, e))?;
            this.variables = serde_json::from_str(&variables_json)
                .map_err(|e| miette::miette!("Error parsing {:?}: {}", variables_json, e))?;
        }

        Ok(this)
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
