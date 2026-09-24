use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowTextLengthW, GetWindowTextW, SetWindowTextW,
};

pub fn hs(s: &str) -> HSTRING {
    HSTRING::from(s)
}

pub fn get_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn set_text(hwnd: HWND, text: &str) {
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(hs(text).as_ptr()));
    }
}

pub fn loword(v: usize) -> u16 {
    (v & 0xffff) as u16
}

pub fn hiword(v: usize) -> u16 {
    ((v >> 16) & 0xffff) as u16
}
