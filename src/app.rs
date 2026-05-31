use std::fs::File;

use anyhow::{Context, Result, bail};
use gpui::{
    App, AppContext, AssetSource, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui_platform::application;

use crate::assets::Assets;
use crate::features::config::{ConfigStore, config_root};
use crate::ui::RootView;

const LOCK_FILE: &str = "boop.lock";
const FONT_PATH: &str = "fonts/Nunito.ttf";

pub fn run() -> Result<()> {
    let lock_path = config_root()?.join(LOCK_FILE);
    let file = File::create(&lock_path)
        .with_context(|| format!("failed to open lock file {}", lock_path.display()))?;
    let mut lock = fd_lock::RwLock::new(file);
    let _guard = match lock.try_write() {
        Ok(guard) => guard,
        Err(_) => bail!("another instance of boop is already running"),
    };

    let store = ConfigStore::new()?;

    application().with_assets(Assets).run(move |cx: &mut App| {
        register_fonts(cx);

        let bounds = Bounds::centered(None, size(px(300.0), px(470.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("boop".into()),
                    ..Default::default()
                }),
                is_resizable: false,
                ..Default::default()
            },
            move |_, cx| {
                let store = store.clone();
                cx.new(move |cx| RootView::new(store, cx))
            },
        )
        .expect("failed to open main window");
        cx.activate(true);
    });

    Ok(())
}

fn register_fonts(cx: &mut App) {
    match Assets.load(FONT_PATH) {
        Ok(Some(font)) => {
            if let Err(error) = cx.text_system().add_fonts(vec![font]) {
                eprintln!("failed to register {FONT_PATH}: {error:#}");
            }
        }
        Ok(None) => eprintln!("font asset {FONT_PATH} not found"),
        Err(error) => eprintln!("failed to load {FONT_PATH}: {error:#}"),
    }
}
