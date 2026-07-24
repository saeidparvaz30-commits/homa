use serde_json::Value;
use std::path::Path;

pub const CONTEXT_WINDOW_TOKENS: f32 = 200_000.0;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Enrichment {
    pub model: Option<String>,
    pub context_pct: Option<f32>,
    pub branch: Option<String>,
    pub last_activity: Option<i64>,
}

pub fn enrich_from_lines(lines: &[String]) -> Enrichment {
    let mut out = Enrichment::default();
    for line in lines.iter().rev() {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if out.branch.is_none() {
            if let Some(b) = v.get("gitBranch").and_then(|b| b.as_str()) {
                if !b.is_empty() {
                    out.branch = Some(b.to_string());
                }
            }
        }
        if v.get("type").and_then(|t| t.as_str()) == Some("assistant") {
            let m = &v["message"];
            out.model = m.get("model").and_then(|x| x.as_str()).map(String::from);
            if let Some(u) = m.get("usage") {
                let g = |k: &str| u.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                let used = g("input_tokens")
                    + g("cache_read_input_tokens")
                    + g("cache_creation_input_tokens");
                out.context_pct = Some((used / CONTEXT_WINDOW_TOKENS * 100.0).clamp(0.0, 100.0));
            }
            return out; // latest assistant turn is authoritative
        }
    }
    out
}

pub fn enrich_from_path(path: &Path) -> Enrichment {
    match std::fs::read_to_string(path) {
        Ok(s) => {
            // Cap: keep only the last 400 lines to bound cost on huge transcripts.
            let lines: Vec<String> = s
                .lines()
                .rev()
                .take(400)
                .map(String::from)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            enrich_from_lines(&lines)
        }
        Err(_) => Enrichment::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<String> {
        include_str!("../tests/fixtures/transcript.jsonl")
            .lines()
            .map(String::from)
            .collect()
    }

    #[test]
    fn pulls_model_and_branch_from_last_assistant() {
        let e = enrich_from_lines(&fixture());
        assert_eq!(e.model.as_deref(), Some("claude-fable-5"));
        assert_eq!(e.branch.as_deref(), Some("main"));
    }

    #[test]
    fn context_pct_sums_input_and_cache_over_window() {
        let e = enrich_from_lines(&fixture());
        // (2 + 40000 + 60000) / 200000 * 100 = 50.001
        let pct = e.context_pct.unwrap();
        assert!((pct - 50.001).abs() < 0.01, "got {pct}");
    }

    #[test]
    fn empty_input_yields_empty_enrichment() {
        let e = enrich_from_lines(&[]);
        assert!(e.model.is_none() && e.context_pct.is_none());
    }
}
