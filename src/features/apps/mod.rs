use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub exe: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub homepage: Option<String>,
}
