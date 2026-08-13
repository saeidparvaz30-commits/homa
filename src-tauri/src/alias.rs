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

use crate::model::AgentState;

pub fn last_segment(cwd: &str) -> String {
    cwd.trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_string()
}

/// Rewrites `name` on every agent to the name the user should see, then
/// disambiguates any duplicates. Runs on every scan, so it must be cheap
/// and its output must be stable for identical input.
pub fn resolve(agents: &mut [AgentState], aliases: &Aliases) {
    for a in agents.iter_mut() {
        let alias = aliases
            .get(&normalize_key(&a.cwd))
            .map(String::as_str)
            .filter(|s| !s.trim().is_empty());
        a.name = match alias {
            Some(s) => s.to_string(),
            None if !a.name.trim().is_empty() => a.name.trim().to_string(),
            None => last_segment(&a.cwd),
        };
    }

    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, a) in agents.iter().enumerate() {
        groups.entry(a.name.clone()).or_default().push(i);
    }
    for (_, mut idxs) in groups {
        if idxs.len() < 2 {
            continue;
        }
        idxs.sort_by(|&x, &y| {
            agents[x]
                .started_at
                .cmp(&agents[y].started_at)
                .then_with(|| agents[x].session_id.cmp(&agents[y].session_id))
        });
        for (n, i) in idxs.into_iter().enumerate() {
            agents[i].name = format!("{} #{}", agents[i].name, n + 1);
        }
    }
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

    use crate::model::{AgentState, AgentStatus};

    fn agent(cwd: &str, claude_name: &str, started_at: i64, sid: &str) -> AgentState {
        AgentState {
            pid: 1,
            session_id: sid.to_string(),
            name: claude_name.to_string(),
            cwd: cwd.to_string(),
            repo: "r".into(),
            branch: None,
            status: AgentStatus::Working,
            raw_status: "busy".into(),
            started_at,
            status_updated_at: 0,
            model: None,
            context_pct: None,
            last_activity: None,
            ended_at: None,
            limited_until: None,
            was_busy_at_limit: false,
            resume_fired: false,
        }
    }

    #[test]
    fn alias_wins_over_claude_name() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "my project".into());
        let mut v = vec![agent("C:\\Foo", "agent-foo-61", 100, "s1")];
        resolve(&mut v, &a);
        assert_eq!(v[0].name, "my project");
    }

    #[test]
    fn falls_back_to_claude_name_when_no_alias() {
        let mut v = vec![agent("C:\\Foo", "agent-foo-61", 100, "s1")];
        resolve(&mut v, &Aliases::new());
        assert_eq!(v[0].name, "agent-foo-61");
    }

    #[test]
    fn falls_back_to_last_path_segment_when_claude_name_blank() {
        let mut v = vec![agent("C:\\Users\\saeid\\Migration Site", "  ", 100, "s1")];
        resolve(&mut v, &Aliases::new());
        assert_eq!(v[0].name, "Migration Site");
    }

    #[test]
    fn two_sessions_in_one_folder_are_suffixed_by_start_order() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "homa".into());
        let mut v = vec![
            agent("C:\\Foo", "agent-foo-62", 200, "s2"),
            agent("c:/foo/", "agent-foo-61", 100, "s1"),
        ];
        resolve(&mut v, &a);
        let older = v.iter().find(|x| x.session_id == "s1").unwrap();
        let newer = v.iter().find(|x| x.session_id == "s2").unwrap();
        assert_eq!(older.name, "homa #1");
        assert_eq!(newer.name, "homa #2");
    }

    #[test]
    fn single_session_gets_no_suffix() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "homa".into());
        let mut v = vec![agent("C:\\Foo", "agent-foo-61", 100, "s1")];
        resolve(&mut v, &a);
        assert_eq!(v[0].name, "homa");
    }

    #[test]
    fn suffix_order_is_stable_when_start_times_tie() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "homa".into());
        let mut v = vec![
            agent("C:\\Foo", "x", 100, "sB"),
            agent("C:\\Foo", "x", 100, "sA"),
        ];
        resolve(&mut v, &a);
        // Ties break on session_id so numbering does not flip between polls.
        assert_eq!(v.iter().find(|x| x.session_id == "sA").unwrap().name, "homa #1");
        assert_eq!(v.iter().find(|x| x.session_id == "sB").unwrap().name, "homa #2");
    }

    #[test]
    fn different_folders_sharing_a_name_are_also_disambiguated() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "work".into());
        a.insert(normalize_key("C:\\Bar"), "work".into());
        let mut v = vec![
            agent("C:\\Foo", "x", 100, "s1"),
            agent("C:\\Bar", "y", 200, "s2"),
        ];
        resolve(&mut v, &a);
        assert_eq!(v[0].name, "work #1");
        assert_eq!(v[1].name, "work #2");
    }
}
