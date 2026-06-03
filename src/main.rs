#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod features;
mod platform;
mod ui;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "boop", version, about = "a hub for completing Discord quests")]
struct Args {}

fn main() {
    Args::parse();
    install_panic_dialog();

    if let Err(error) = app::run() {
        // release builds have no console (windows_subsystem = "windows"), so a
        // returned error would vanish. surface it in a native dialog instead.
        platform::show_error("boop failed to start", &format!("{error:#}"));
        std::process::exit(1);
    }
}

/// route panics to a native dialog so crashes are visible without a console.
/// keeps the default hook so backtraces still reach stderr in debug builds.
fn install_panic_dialog() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        platform::show_error("boop crashed", &info.to_string());
        default_hook(info);
    }));
}
