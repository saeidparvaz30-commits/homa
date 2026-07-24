use std::collections::HashSet;
use std::sync::Mutex;

static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Append any not-yet-seen raw status string to observed-statuses.log, once per
/// distinct value per run. Used to enumerate the real status enum empirically.
pub fn record(raw: &str) {
    let raw = raw.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return;
    }
    let mut g = SEEN.lock().unwrap();
    let set = g.get_or_insert_with(HashSet::new);
    if set.insert(raw.clone()) {
        let p = dirs::config_dir()
            .unwrap_or_default()
            .join("homa")
            .join("observed-statuses.log");
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{}", raw);
        }
    }
}
