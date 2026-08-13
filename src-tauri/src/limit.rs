use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum LimitKind {
    Session { reset_h: u32, reset_m: u32 },
    Credit,
    Login,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitEvent {
    pub kind: LimitKind,
}

/// Scans newest-first. A synthetic limit message is fresh only if no real
/// user or assistant turn was written after it; otherwise the user already
/// moved past it and it must not retrigger anything.
pub fn detect(lines: &[String]) -> Option<LimitEvent> {
    for line in lines.iter().rev() {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if t != "assistant" && t != "user" {
            continue;
        }
        let synthetic =
            v["message"].get("model").and_then(|m| m.as_str()) == Some("<synthetic>");
        if !synthetic {
            return None; // real activity newer than any limit line
        }
        let text = v["message"]["content"]
            .as_array()
            .and_then(|c| c.first())
            .and_then(|b| b.get("text"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if text.contains("limit") && text.contains("resets") {
            let (reset_h, reset_m) = parse_reset(text)?;
            return Some(LimitEvent {
                kind: LimitKind::Session { reset_h, reset_m },
            });
        }
        if text.contains("Credit balance") {
            return Some(LimitEvent { kind: LimitKind::Credit });
        }
        if text.contains("Login expired") {
            return Some(LimitEvent { kind: LimitKind::Login });
        }
        // Other synthetic lines ("No response requested.") are neither
        // activity nor limits: keep scanning older lines.
    }
    None
}

/// Parses "resets 12:40am" / "resets 3pm" into 24h (hour, minute).
pub fn parse_reset(text: &str) -> Option<(u32, u32)> {
    let idx = text.find("resets ")?;
    let rest = &text[idx + 7..];
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':' || *c == 'a' || *c == 'p' || *c == 'm')
        .collect();
    let lower = token.to_ascii_lowercase();
    let pm = lower.ends_with("pm");
    let am = lower.ends_with("am");
    if !pm && !am {
        return None;
    }
    let hm = &lower[..lower.len() - 2];
    let (h_str, m_str) = match hm.split_once(':') {
        Some((h, m)) => (h, m),
        None => (hm, "0"),
    };
    let mut h: u32 = h_str.parse().ok()?;
    let m: u32 = m_str.parse().ok()?;
    if h == 12 {
        h = 0;
    }
    if pm {
        h += 12;
    }
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

/// Pure next-occurrence arithmetic: the reset is a local wall clock time,
/// so the delta to it is computed in local seconds and applied to epoch now.
pub fn resets_at_ms(now_ms: i64, local_secs_since_midnight: i64, h: u32, m: u32) -> i64 {
    let target = (h as i64) * 3600 + (m as i64) * 60;
    let mut delta = target - local_secs_since_midnight;
    if delta <= 0 {
        delta += 24 * 3600;
    }
    now_ms + delta * 1000
}

pub fn last_ai_title(lines: &[String]) -> Option<String> {
    for line in lines.iter().rev() {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if v.get("type").and_then(|x| x.as_str()) == Some("ai-title") {
                return v.get("aiTitle").and_then(|x| x.as_str()).map(String::from);
            }
        }
    }
    None
}

/// Reads at most `max_bytes` from the end of the file, split into whole lines
/// (a partial first line is dropped). Transcripts grow to many MB; the poll
/// loop touches them every 2s, so it must never read the whole file.
pub fn read_tail(path: &Path, max_bytes: u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max_bytes);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = String::new();
    if f.read_to_string(&mut buf).is_err() {
        return Vec::new();
    }
    let mut lines: Vec<String> = buf.lines().map(String::from).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    lines
}

/// Seconds since local midnight, from the OS local clock. Kept thin so the
/// arithmetic in `resets_at_ms` stays pure and testable.
pub fn local_secs_since_midnight() -> i64 {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        (st.wHour as i64) * 3600 + (st.wMinute as i64) * 60 + st.wSecond as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(text: &str) -> String {
        format!(
            r#"{{"type":"assistant","message":{{"model":"<synthetic>","role":"assistant","content":[{{"type":"text","text":"{text}"}}]}}}}"#
        )
    }
    fn real_assistant() -> String {
        r#"{"type":"assistant","message":{"model":"claude-opus-4-8","role":"assistant","content":[{"type":"text","text":"done"}]}}"#.into()
    }
    fn user_line() -> String {
        r#"{"type":"user","message":{"role":"user","content":"go on"}}"#.into()
    }

    #[test]
    fn detects_session_limit_with_reset_time() {
        let lines = vec![
            real_assistant(),
            synth("You've hit your session limit \u{b7} resets 12:40am (Europe/Oslo)"),
        ];
        match detect(&lines) {
            Some(LimitEvent {
                kind: LimitKind::Session { reset_h: 0, reset_m: 40 },
            }) => {}
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn newer_activity_makes_the_limit_stale() {
        let lines = vec![
            synth("You've hit your session limit \u{b7} resets 4:00pm (Europe/Oslo)"),
            user_line(),
        ];
        assert!(detect(&lines).is_none());
        let lines2 = vec![
            synth("You've hit your session limit \u{b7} resets 4:00pm (Europe/Oslo)"),
            real_assistant(),
        ];
        assert!(detect(&lines2).is_none());
    }

    #[test]
    fn no_response_requested_synthetic_is_ignored_not_activity() {
        let lines = vec![
            synth("You've hit your session limit \u{b7} resets 4:00pm (Europe/Oslo)"),
            synth("No response requested."),
        ];
        assert!(detect(&lines).is_some());
    }

    #[test]
    fn credit_and_login_variants_detected_without_reset() {
        assert!(matches!(
            detect(&[synth("Credit balance is too low")]),
            Some(LimitEvent { kind: LimitKind::Credit })
        ));
        assert!(matches!(
            detect(&[synth("Login expired \u{b7} Please run /login")]),
            Some(LimitEvent { kind: LimitKind::Login })
        ));
    }

    #[test]
    fn parse_reset_handles_am_pm_and_no_minutes() {
        assert_eq!(parse_reset("resets 12:40am (Europe/Oslo)"), Some((0, 40)));
        assert_eq!(parse_reset("resets 4:05pm (Europe/Oslo)"), Some((16, 5)));
        assert_eq!(parse_reset("resets 3pm"), Some((15, 0)));
        assert_eq!(parse_reset("resets 12pm"), Some((12, 0)));
        assert_eq!(parse_reset("no time here"), None);
    }

    #[test]
    fn resets_at_rolls_forward_to_next_occurrence() {
        // Local time 23:00:00; reset 12:40am -> 1h40m ahead.
        let now_ms = 1_000_000_000;
        let secs = 23 * 3600;
        assert_eq!(resets_at_ms(now_ms, secs, 0, 40), now_ms + (100 * 60) * 1000);
        // Local 01:00, reset 12:40am -> tomorrow.
        let secs2 = 3600;
        assert_eq!(
            resets_at_ms(now_ms, secs2, 0, 40),
            now_ms + ((24 * 3600 - 3600) + 40 * 60) as i64 * 1000
        );
    }

    #[test]
    fn last_ai_title_wins() {
        let lines = vec![
            r#"{"type":"ai-title","aiTitle":"old title"}"#.to_string(),
            real_assistant(),
            r#"{"type":"ai-title","aiTitle":"new title"}"#.to_string(),
        ];
        assert_eq!(last_ai_title(&lines).as_deref(), Some("new title"));
    }

    #[test]
    fn read_tail_returns_last_lines_of_large_file() {
        let p = std::env::temp_dir().join("homa-limit-tail-test.jsonl");
        let big: String = (0..5000).map(|i| format!("line{i}\n")).collect();
        std::fs::write(&p, big).unwrap();
        let lines = read_tail(&p, 4096);
        assert!(lines.len() > 1);
        assert_eq!(lines.last().unwrap(), "line4999");
        assert!(read_tail(Path::new("C:\\does\\not\\exist.jsonl"), 4096).is_empty());
    }
}
