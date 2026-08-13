use crate::terminal;

fn send_unicode_and_enter(text: &str) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_RETURN,
    };
    unsafe {
        let mut inputs: Vec<INPUT> = Vec::new();
        for ch in text.encode_utf16() {
            for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
                let mut i: INPUT = std::mem::zeroed();
                i.r#type = INPUT_KEYBOARD;
                i.Anonymous.ki.wScan = ch;
                i.Anonymous.ki.dwFlags = flags;
                inputs.push(i);
            }
        }
        for flags in [0u32, KEYEVENTF_KEYUP] {
            let mut i: INPUT = std::mem::zeroed();
            i.r#type = INPUT_KEYBOARD;
            i.Anonymous.ki.wVk = VK_RETURN;
            i.Anonymous.ki.dwFlags = flags;
            inputs.push(i);
        }
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// Focus first, give the terminal a beat to accept input, then type.
pub fn nudge(hwnd: isize) {
    terminal::focus_hwnd(hwnd);
    std::thread::sleep(std::time::Duration::from_millis(250));
    send_unicode_and_enter("continue");
}
