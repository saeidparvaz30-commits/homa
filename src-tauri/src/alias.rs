use serde_json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type Aliases = BTreeMap<String, String>;

/// Windows paths are case insensitive and reach us spelled inconsistently
/// depending on how the session was launched, so keys are folded before use.
pub fn normalize_key(cwd: &str) -> String {
    let mut s = cwd.trim().replace('/', "\\").to_lowercase();
    while s.len() > 1 && s.ends_with('\\') {
        s.pop();
    }
    s
}

pub fn aliases_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("homa")
        .join("aliases.json")
}

pub fn load_from(p: &Path) -> Aliases {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn load() -> Aliases {
    load_from(&aliases_path())
}

pub fn save_to(p: &Path, a: &Aliases) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    // Write to a sibling temp file and rename over the target: on Windows
    // rename replaces atomically, so a crash mid write cannot truncate the store.
    let tmp = p.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(a)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, p)
}

pub fn set_in(p: &Path, cwd: &str, name: &str) -> std::io::Result<Aliases> {
    let mut a = load_from(p);
    let key = normalize_key(cwd);
    let n = name.trim();
    if n.is_empty() {
        a.remove(&key);
    } else {
        a.insert(key, n.to_string());
    }
    save_to(p, &a)?;
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("homa-alias-test-{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn normalize_is_case_and_separator_insensitive() {
        let a = normalize_key("C:\\Users\\Saeid\\Desktop\\Migration Site");
        let b = normalize_key("c:/users/saeid/desktop/migration site");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_strips_trailing_separators() {
        assert_eq!(normalize_key("c:\\foo\\bar\\"), normalize_key("c:\\foo\\bar"));
    }

    #[test]
    fn missing_file_loads_empty() {
        let p = tmpdir("missing").join("aliases.json");
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn corrupt_file_loads_empty_and_does_not_panic() {
        let p = tmpdir("corrupt").join("aliases.json");
        std::fs::write(&p, b"{not json").unwrap();
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn set_then_load_round_trips() {
        let p = tmpdir("roundtrip").join("aliases.json");
        set_in(&p, "C:\\Foo\\Bar", "my project").unwrap();
        let a = load_from(&p);
        assert_eq!(a.get(&normalize_key("c:/foo/bar")).map(String::as_str), Some("my project"));
    }

    #[test]
    fn set_trims_whitespace() {
        let p = tmpdir("trim").join("aliases.json");
        set_in(&p, "C:\\Foo", "  spaced  ").unwrap();
        assert_eq!(load_from(&p).get(&normalize_key("c:\\foo")).map(String::as_str), Some("spaced"));
    }

    #[test]
    fn empty_name_removes_entry_rather_than_storing_blank() {
        let p = tmpdir("clear").join("aliases.json");
        set_in(&p, "C:\\Foo", "temp").unwrap();
        set_in(&p, "C:\\Foo", "   ").unwrap();
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let p = tmpdir("overwrite").join("aliases.json");
        set_in(&p, "C:\\Foo", "first").unwrap();
        set_in(&p, "C:\\Foo", "second").unwrap();
        assert_eq!(load_from(&p).get(&normalize_key("c:\\foo")).map(String::as_str), Some("second"));
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let p = tmpdir("mkparent").join("nested").join("aliases.json");
        set_in(&p, "C:\\Foo", "ok").unwrap();
        assert!(p.exists());
    }
}
