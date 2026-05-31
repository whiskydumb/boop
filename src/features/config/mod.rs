use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::features::apps::AppEntry;

mod store;

pub use store::ConfigStore;

const CONFIG_FILE: &str = "config.toml";
const DEFAULT_APPS_DIR: &str = "apps";

// the default config shipped on first run is the canonical list kept under
// .github/assets; baked into the binary at build time (relative to the crate
// root so the include works regardless of this file's location). editing that
// file triggers a rebuild.
const DEFAULT_CONFIG_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.github/assets/config.toml"
));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_apps_dir")]
    pub apps_dir: PathBuf,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub catalogs: Vec<String>,
    #[serde(skip)]
    root: PathBuf,
}

fn default_apps_dir() -> PathBuf {
    PathBuf::from(DEFAULT_APPS_DIR)
}

impl Config {
    pub fn load() -> Result<Self> {
        let root = config_root().context("failed to locate the config directory")?;
        let path = root.join(CONFIG_FILE);

        if !path.exists() {
            bootstrap(&root, &path)?;
        }

        Self::read(&root, &path)
    }

    pub fn reload() -> Result<Self> {
        let root = config_root().context("failed to locate the config directory")?;
        let path = root.join(CONFIG_FILE);
        Self::read(&root, &path)
    }

    fn read(root: &Path, path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut config: Config = toml::from_str(&raw)
            .with_context(|| format!("malformed config at {}", path.display()))?;
        config.root = root.to_path_buf();
        Ok(config)
    }

    pub fn apps_dir_abs(&self) -> PathBuf {
        self.root.join(&self.apps_dir)
    }

    pub fn resolve_exe(&self, entry: &AppEntry) -> PathBuf {
        resolve(&self.apps_dir_abs(), &entry.exe)
    }

    pub fn resolve_cwd(&self, entry: &AppEntry) -> Option<PathBuf> {
        match &entry.cwd {
            Some(cwd) => Some(resolve(&self.apps_dir_abs(), cwd)),
            None => self
                .resolve_exe(entry)
                .parent()
                .map(|parent| parent.to_path_buf()),
        }
    }

    pub fn apps(&self) -> &[AppEntry] {
        &self.apps
    }
}

fn bootstrap(root: &Path, path: &Path) -> Result<()> {
    fs::write(path, DEFAULT_CONFIG_TEMPLATE)
        .with_context(|| format!("failed to write default config to {}", path.display()))?;

    let apps_dir = root.join(DEFAULT_APPS_DIR);
    fs::create_dir_all(&apps_dir)
        .with_context(|| format!("failed to create apps dir at {}", apps_dir.display()))?;

    Ok(())
}

fn resolve(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

pub fn config_root() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot determine current executable path")?;
    let dir = exe.parent().context("executable has no parent directory")?;
    Ok(dir.to_path_buf())
}
