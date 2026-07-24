pub fn repo_from_cwd(cwd: &str) -> String {
    let seg = cwd
        .trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .trim();
    if seg.is_empty() {
        "unknown".to_string()
    } else {
        seg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_final_windows_segment() {
        assert_eq!(
            repo_from_cwd(r"C:\Users\saeid\Desktop\Agent Simorgh\projects"),
            "projects"
        );
    }

    #[test]
    fn takes_final_unix_segment() {
        assert_eq!(repo_from_cwd("/c/Users/saeid/Homa"), "Homa");
    }

    #[test]
    fn empty_is_unknown() {
        assert_eq!(repo_from_cwd(""), "unknown");
    }
}
