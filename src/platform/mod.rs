#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::{open_url, show_error};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(not(target_os = "windows"))]
pub fn show_error(title: &str, message: &str) {
    eprintln!("{title}: {message}");
}

#[cfg(not(target_os = "windows"))]
pub fn open_url(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(url).spawn();
}
