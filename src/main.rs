#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod features;
mod platform;
mod ui;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "boop", version, about = "a hub for completing Discord quests")]
struct Args {}

fn main() -> Result<()> {
    Args::parse();
    app::run()
}
