use std::path::{Path, PathBuf};

use miette::Context;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {}

impl State {
    fn data_dir(config_path: &Path) -> miette::Result<PathBuf> {
        let parent = config_path.parent().context("Invalid config path")?;
        let data_dir = parent.join(".inq");
        Ok(data_dir)
    }

    pub fn load(config_path: impl AsRef<Path>) -> miette::Result<Self> {
        Ok(Self {})
    }

    pub fn save(&self, config_path: impl AsRef<Path>) -> miette::Result<()> {
        todo!();
        // let data_dir = Self::data_dir(config_path.as_ref())?;

        // let dir_existed = data_dir.exists();

        // std::fs::create_dir_all(&data_dir)
        //     .map_err(|e| miette::miette!("Error creating {:?}: {}", data_dir, e))?;

        // if !dir_existed {
        //     std::fs::write(data_dir.join(".gitignore"), "*\n")
        //         .map_err(|e| miette::miette!("Error writing .inq/.gitignore: {}", e))?;
        // }

        // let variables = data_dir.join("variables.json");
        // let variables = std::fs::File::create(variables)
        //     .map_err(|e| miette::miette!("Error creating .inq/variables.json: {}", e))?;
        // let variables = BufWriter::new(variables);
        // serde_json::to_writer_pretty(variables, &self.variables)
        //     .map_err(|e| miette::miette!("Error writing .inq/variables.json: {}", e))?;

        Ok(())
    }
}
