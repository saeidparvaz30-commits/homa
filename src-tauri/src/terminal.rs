use sysinfo::System;

pub struct Win {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
}

/// Innermost first: [session pid, shell pid, terminal pid, ...].
pub fn ancestor_pids(pid: u32) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
    let mut out = vec![pid];
    let mut cur = sysinfo::Pid::from_u32(pid);
    for _ in 0..16 {
        let Some(p) = sys.process(cur).and_then(|p| p.parent()) else {
            break;
        };
        out.push(p.as_u32());
        cur = p;
    }
    out
}

pub fn list_windows() -> Vec<Win> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        let out = &mut *(lparam as *mut Vec<Win>);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if n > 0 {
            out.push(Win {
                hwnd: hwnd as isize,
                pid,
                title: String::from_utf16_lossy(&buf[..n as usize]),
            });
        }
        1
    }
    let mut out: Vec<Win> = Vec::new();
    unsafe {
        EnumWindows(Some(cb), &mut out as *mut _ as isize);
    }
    out
}

/// Title match beats ancestry depth: several sessions can share one terminal
/// process, and only the title tells their windows apart. Among plain
/// ancestor windows, innermost first.
pub fn pick_window(wins: &[Win], ancestors: &[u32], ai_title: Option<&str>) -> Option<isize> {
    if let Some(t) = ai_title {
        if let Some(w) = wins
            .iter()
            .find(|w| ancestors.contains(&w.pid) && w.title.ends_with(t))
        {
            return Some(w.hwnd);
        }
    }
    for pid in ancestors {
        if let Some(w) = wins.iter().find(|w| w.pid == *pid) {
            return Some(w.hwnd);
        }
    }
    None
}

pub fn resolve_hwnd(pid: u32, ai_title: Option<&str>) -> Option<isize> {
    pick_window(&list_windows(), &ancestor_pids(pid), ai_title)
}

pub fn focus_hwnd(hwnd: isize) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            ShowWindow(hwnd as _, SW_RESTORE);
        }
        // A background process may not steal foreground; a synthetic Alt tap
        // marks this thread as input-active so SetForegroundWindow is honored.
        let mut alt: [INPUT; 2] = std::mem::zeroed();
        for (i, flags) in [(0usize, 0u32), (1usize, KEYEVENTF_KEYUP)] {
            alt[i].r#type = INPUT_KEYBOARD;
            alt[i].Anonymous.ki.wVk = VK_MENU;
            alt[i].Anonymous.ki.dwFlags = flags;
        }
        SendInput(2, alt.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        SetForegroundWindow(hwnd as _);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(hwnd: isize, pid: u32, title: &str) -> Win {
        Win { hwnd, pid, title: title.into() }
    }

    #[test]
    fn title_match_on_ancestor_window_wins() {
        let wins = vec![
            w(1, 100, "\u{25d0} other session"),
            w(2, 100, "\u{25d0} my task title"),
            w(3, 999, "my task title"), // right title, not an ancestor
        ];
        assert_eq!(pick_window(&wins, &[50, 100], Some("my task title")), Some(2));
    }

    #[test]
    fn falls_back_to_innermost_ancestor_window_without_title_match() {
        let wins = vec![w(9, 200, "whatever"), w(8, 100, "shell")];
        assert_eq!(pick_window(&wins, &[100, 200], None), Some(8));
        assert_eq!(pick_window(&wins, &[300, 200], Some("nope")), Some(9));
    }

    #[test]
    fn no_candidates_yields_none() {
        assert_eq!(pick_window(&[], &[1, 2], Some("x")), None);
    }

    #[test]
    fn own_process_ancestry_is_nonempty_and_starts_with_self() {
        let pids = ancestor_pids(std::process::id());
        assert_eq!(pids.first().copied(), Some(std::process::id()));
        assert!(pids.len() >= 2, "a test process always has a parent");
    }

    #[test]
    fn local_secs_since_midnight_is_in_range() {
        let s = crate::limit::local_secs_since_midnight();
        assert!((0..86_400).contains(&s));
    }
}
