use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SessionFile {
    pub pid: u32,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub cwd: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "startedAt", default)]
    pub started_at: i64,
    #[serde(rename = "statusUpdatedAt", default)]
    pub status_updated_at: i64,
    #[serde(default)]
    pub version: String,
}

pub fn parse_session(bytes: &[u8]) -> Result<SessionFile, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    const BUSY: &str = r#"{"pid":13732,"sessionId":"1fe8e5c7","cwd":"C:\\Users\\saeid\\Desktop\\Agent Simorgh","startedAt":1784898651809,"version":"2.1.218","kind":"interactive","name":"agent-simorgh-61","status":"busy","updatedAt":1784900977968,"statusUpdatedAt":1784900977968}"#;

    #[test]
    fn parses_core_fields() {
        let s = parse_session(BUSY.as_bytes()).unwrap();
        assert_eq!(s.pid, 13732);
        assert_eq!(s.name, "agent-simorgh-61");
        assert_eq!(s.status, "busy");
        assert_eq!(s.status_updated_at, 1784900977968);
    }

    #[test]
    fn partial_json_errors_not_panics() {
        assert!(parse_session(b"{\"pid\":1,").is_err());
    }
}
