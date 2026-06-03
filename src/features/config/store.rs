use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use anyhow::{Context, Result};
use arc_swap::{ArcSwap, ArcSwapOption};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::{CONFIG_FILE, Config, config_root};

pub struct ConfigStore {
    current: ArcSwap<Config>,
    error: ArcSwapOption<String>,
    dirty: AtomicBool,
    _watcher: Mutex<Option<RecommendedWatcher>>,
}

impl ConfigStore {
    pub fn new() -> Result<Arc<Self>> {
        let config = Config::load()?;
        let store = Arc::new(Self {
            current: ArcSwap::from_pointee(config),
            error: ArcSwapOption::empty(),
            dirty: AtomicBool::new(false),
            _watcher: Mutex::new(None),
        });

        let watcher =
            build_watcher(Arc::downgrade(&store)).context("failed to start config watcher")?;
        *store._watcher.lock().expect("watcher mutex poisoned") = Some(watcher);

        Ok(store)
    }

    pub fn snapshot(&self) -> Arc<Config> {
        self.current.load_full()
    }

    pub fn error(&self) -> Option<Arc<String>> {
        self.error.load_full()
    }

    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }
}

fn build_watcher(store: Weak<ConfigStore>) -> Result<RecommendedWatcher> {
    let root = config_root()?;

    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else {
            return;
        };
        // watching the directory (not the file directly) survives atomic
        // saves that replace config.toml via rename; filter to our file.
        if !event.paths.iter().any(|path| path.ends_with(CONFIG_FILE)) {
            return;
        }
        let Some(store) = store.upgrade() else {
            return;
        };
        match Config::reload() {
            Ok(config) => {
                store.current.store(Arc::new(config));
                store.error.store(None);
            }
            // keep the last good config live, but record why the reload failed
            // so the UI can surface it instead of silently ignoring the edit.
            Err(error) => store.error.store(Some(Arc::new(format!("{error:#}")))),
        }
        store.dirty.store(true, Ordering::Release);
    })?;

    watcher
        .watch(&root, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", root.display()))?;

    Ok(watcher)
}
