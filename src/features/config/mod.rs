use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashSet;

use anyhow::{Context, Result, bail};
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

const DEFAULT_QUEST_MINUTES: u32 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_apps_dir")]
    pub apps_dir: PathBuf,
    #[serde(default = "default_quest_minutes")]
    pub quest_minutes: u32,
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

fn default_quest_minutes() -> u32 {
    DEFAULT_QUEST_MINUTES
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
        let raw = fs::read_to_string(path).with_context(|| {
            format!(
                "failed to read {} (the file must be UTF-8 text)",
                path.display()
            )
        })?;
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
        let mut config: Config = parse_lenient(raw)
            .with_context(|| format!("malformed config at {}", path.display()))?;
        config.root = root.to_path_buf();
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.quest_minutes == 0 {
            bail!("quest_minutes must be at least 1");
        }
        let mut seen = HashSet::with_capacity(self.apps.len());
        for app in &self.apps {
            if app.id.trim().is_empty() {
                bail!(
                    "app '{}' has an empty id; every entry needs a unique id",
                    app.name
                );
            }
            if !seen.insert(app.id.as_str()) {
                bail!("duplicate app id '{}'; ids must be unique", app.id);
            }
        }
        Ok(())
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

fn parse_lenient(raw: &str) -> std::result::Result<Config, toml::de::Error> {
    match toml::from_str(raw) {
        Ok(config) => Ok(config),
        Err(strict_err) => toml::from_str(&lenient_toml(raw)).map_err(|_| strict_err),
    }
}

const VALID_ESCAPES: &[char] = &['"', '\\', '/', 'b', 'f', 'n', 'r', 't', 'u', 'U'];

fn lenient_toml(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let n = chars.len();
    let triple = |i: usize, q: char| {
        chars.get(i) == Some(&q) && chars.get(i + 1) == Some(&q) && chars.get(i + 2) == Some(&q)
    };
    let mut out = String::with_capacity(raw.len() + 16);
    let mut i = 0;
    while i < n {
        match chars[i] {
            // comment: copy to end of line.
            '#' => {
                while i < n && chars[i] != '\n' {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            // literal strings have no escapes -- copy verbatim, backslashes intact.
            '\'' if triple(i, '\'') => {
                out.push_str("'''");
                i += 3;
                while i < n && !triple(i, '\'') {
                    out.push(chars[i]);
                    i += 1;
                }
                if i < n {
                    out.push_str("'''");
                    i += 3;
                }
            }
            '\'' => {
                out.push('\'');
                i += 1;
                while i < n && chars[i] != '\'' {
                    out.push(chars[i]);
                    i += 1;
                }
                if i < n {
                    out.push('\'');
                    i += 1;
                }
            }
            // multiline basic strings are rare here; copy verbatim, don't rewrite.
            '"' if triple(i, '"') => {
                out.push_str("\"\"\"");
                i += 3;
                while i < n && !triple(i, '"') {
                    out.push(chars[i]);
                    i += 1;
                }
                if i < n {
                    out.push_str("\"\"\"");
                    i += 3;
                }
            }
            // basic string: fix invalid backslash escapes as we copy.
            '"' => {
                out.push('"');
                i += 1;
                while i < n && chars[i] != '"' {
                    if chars[i] == '\\' {
                        match chars.get(i + 1) {
                            // valid escape (incl. \" which is not a closer): keep both.
                            Some(next) if VALID_ESCAPES.contains(next) => {
                                out.push('\\');
                                out.push(*next);
                                i += 2;
                            }
                            // invalid escape (e.g. a windows path's \E): double the backslash.
                            _ => {
                                out.push_str("\\\\");
                                i += 1;
                            }
                        }
                    } else {
                        out.push(chars[i]);
                        i += 1;
                    }
                }
                if i < n {
                    out.push('"');
                    i += 1;
                }
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescues_backslash_windows_path() {
        // the exact entry that failed for the user: backslashes in a basic string.
        let raw = "[[apps]]\nid = \"dna\"\nname = \"DNA\"\nexe = \"Duet Night Abyss\\EM-Win64-Shipping.exe\"\n";
        let config = parse_lenient(raw).expect("lenient fallback should rescue the path");
        assert_eq!(
            config.apps[0].exe,
            PathBuf::from(r"Duet Night Abyss\EM-Win64-Shipping.exe")
        );
    }

    #[test]
    fn leaves_forward_slash_config_intact() {
        let raw = "[[apps]]\nid = \"x\"\nname = \"X\"\nexe = \"Win64/wwm.exe\"\n";
        let config = parse_lenient(raw).expect("valid config parses strictly");
        assert_eq!(config.apps[0].exe, PathBuf::from("Win64/wwm.exe"));
    }

    #[test]
    fn preserves_valid_escapes_and_literal_strings() {
        // \t is a valid escape; single-quoted strings are literal -- both untouched.
        let input = "a = \"tab\\there\"\nb = 'C:\\raw\\path'\n";
        assert_eq!(lenient_toml(input), input);
    }

    #[test]
    fn doubles_only_invalid_escapes() {
        assert_eq!(lenient_toml("\"a\\Eb\""), "\"a\\\\Eb\"");
    }

    #[test]
    fn quest_minutes_defaults_when_omitted() {
        let config: Config = toml::from_str("apps = []").unwrap();
        assert_eq!(config.quest_minutes, DEFAULT_QUEST_MINUTES);
    }

    #[test]
    fn validate_rejects_zero_quest_minutes() {
        let config: Config = toml::from_str("quest_minutes = 0\napps = []").unwrap();
        assert!(config.validate().is_err());
    }
}
