#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use std::mem::zeroed;
    use std::ptr::{null, null_mut};

    use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
    use winapi::shared::windef::HWND;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostQuitMessage,
        RegisterClassW, SW_SHOWNOACTIVATE, ShowWindow, TranslateMessage, WM_DESTROY, WNDCLASSW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    };

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            if msg == WM_DESTROY {
                PostQuitMessage(0);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let title = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "boop stub".to_string());

    unsafe {
        let instance = GetModuleHandleW(null());
        let class_name = wide("boop_stub_window");

        let mut class: WNDCLASSW = zeroed();
        class.lpfnWndProc = Some(wndproc);
        class.hInstance = instance;
        class.lpszClassName = class_name.as_ptr();
        RegisterClassW(&class);

        let title_w = wide(&title);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            title_w.as_ptr(),
            WS_POPUP,
            -32000,
            -32000,
            480,
            320,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    use std::{thread, time::Duration};

    // no windowing on non-windows targets; just park until killed
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
