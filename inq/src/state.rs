use std::{
    collections::HashMap,
    io::BufWriter,
    path::{Path, PathBuf},
    rc::Rc,
};

use chrono::{DateTime, Utc};
use inq_lang::IStr;
use miette::{Context, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::config::Config;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct PersistedVariable {
    pub value: IStr,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct State {
    data_dir: PathBuf,
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

    fn cleanup_vars(&mut self) {
        self.variables
            .retain(|_, v| v.expires_at.is_none_or(|e| e > Utc::now()));
    }

    pub fn load(config_path: impl AsRef<Path>) -> miette::Result<Self> {
        let data_dir = Self::data_dir(config_path.as_ref())?;

        let mut this = Self {
            variables: Self::load_data_file_or(&data_dir, "variables.json", Default::default)?,
            data_dir,
        };
        this.cleanup_vars();
        Ok(this)
    }

    pub fn apply_variables(&self, config: Rc<Config>) -> miette::Result<()> {
        for v in &config.persisted_vars {
            if let Some(var) = self.variables.get(&v.as_istr()) {
                assert!(var.expires_at.is_none_or(|e| e > Utc::now()));
                config
                    .engine
                    .global()
                    .set_variable(v.as_istr(), var.value.clone(), false);
            }
        }
        Ok(())
    }

    pub fn update_variables(&mut self, config: &Config) -> miette::Result<()> {
        for var in &config.persisted_vars {
            let Some(v) = config
                .engine
                .global()
                .get_evaluated_variable(&var.as_istr())?
            else {
                continue;
            };
            if let Some(s) = v.downcast::<IStr>() {
                self.variables.insert(
                    var.as_istr(),
                    PersistedVariable {
                        value: s,
                        expires_at: None,
                    },
                );
            } else {
                bail! {
                    labels = vec![var.span.with_label("Defined here")],
                    "Persisted variables must be strings"
                }
            }
        }
        Ok(())
    }

    fn make_data_dir(&self) -> miette::Result<()> {
        let dir_existed = self.data_dir.exists();

        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| miette::miette!("Error creating {:?}: {}", self.data_dir, e))?;

        if !dir_existed {
            std::fs::write(self.data_dir.join(".gitignore"), "*\n")
                .map_err(|e| miette::miette!("Error writing .inq/.gitignore: {}", e))?;
        }

        Ok(())
    }

    pub fn save(&self) -> miette::Result<()> {
        self.make_data_dir()?;

        let variables = self.data_dir.join("variables.json");
        let variables = std::fs::File::create(variables)
            .map_err(|e| miette::miette!("Error creating .inq/variables.json: {}", e))?;
        let variables = BufWriter::new(variables);
        serde_json::to_writer_pretty(variables, &self.variables)
            .map_err(|e| miette::miette!("Error writing .inq/variables.json: {}", e))?;

        Ok(())
    }

    pub(crate) fn persisted_variables(&self) -> impl Iterator<Item = (IStr, PersistedVariable)> {
        self.variables.iter().map(|(k, v)| (k.clone(), v.clone()))
    }

    pub(crate) fn get_persisted_var(&self, name: impl AsRef<str>) -> Option<PersistedVariable> {
        self.variables.get(name.as_ref()).cloned()
    }

    pub(crate) fn set_persisted_var(
        &mut self,
        name: impl Into<IStr>,
        value: impl Into<IStr>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Option<PersistedVariable> {
        self.variables.insert(
            name.into(),
            PersistedVariable {
                value: value.into(),
                expires_at,
            },
        )
    }
}
