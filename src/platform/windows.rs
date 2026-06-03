use std::iter::once;
use std::ptr::null_mut;

use winapi::um::winuser::{MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(once(0)).collect()
}

pub fn show_error(title: &str, message: &str) {
    let caption = wide(title);
    let body = wide(message);
    unsafe {
        MessageBoxW(
            null_mut(),
            body.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}

pub fn open_url(url: &str) {
    let _ = std::process::Command::new("explorer").arg(url).spawn();
}
