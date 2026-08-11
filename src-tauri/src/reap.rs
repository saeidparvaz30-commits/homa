use crate::model::{AgentState, AgentStatus};

pub const ENDED_TTL_MS: i64 = 10_000;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Stamps the moment a session was first seen dead, carries that stamp across
/// polls, and removes rows that have been dead longer than `ttl_ms`.
pub fn reap(prev: &[AgentState], next: &mut Vec<AgentState>, now: i64, ttl_ms: i64) {
    for a in next.iter_mut() {
        if a.status != AgentStatus::Ended {
            a.ended_at = None;
            continue;
        }
        let carried = prev
            .iter()
            .find(|p| p.session_id == a.session_id)
            .and_then(|p| p.ended_at);
        a.ended_at = Some(carried.unwrap_or(now));
    }
    next.retain(|a| match (a.status, a.ended_at) {
        (AgentStatus::Ended, Some(t)) => now - t <= ttl_ms,
        _ => true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(sid: &str, status: AgentStatus, ended_at: Option<i64>) -> AgentState {
        AgentState {
            pid: 1,
            session_id: sid.to_string(),
            name: sid.to_string(),
            cwd: "c".into(),
            repo: "r".into(),
            branch: None,
            status,
            raw_status: "x".into(),
            started_at: 0,
            status_updated_at: 0,
            model: None,
            context_pct: None,
            last_activity: None,
            ended_at,
        }
    }

    #[test]
    fn stamps_ended_at_on_first_sighting() {
        let prev = vec![agent("s1", AgentStatus::Working, None)];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 5_000, ENDED_TTL_MS);
        assert_eq!(next[0].ended_at, Some(5_000));
    }

    #[test]
    fn carries_ended_at_forward_across_polls() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 9_000, ENDED_TTL_MS);
        assert_eq!(next[0].ended_at, Some(5_000), "must not restamp each poll");
    }

    #[test]
    fn keeps_ended_session_inside_ttl() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 14_000, ENDED_TTL_MS);
        assert_eq!(next.len(), 1);
    }

    #[test]
    fn drops_ended_session_past_ttl() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 15_001, ENDED_TTL_MS);
        assert!(next.is_empty());
    }

    #[test]
    fn never_drops_a_live_session() {
        let prev = vec![agent("s1", AgentStatus::Working, None)];
        let mut next = vec![agent("s1", AgentStatus::Working, None)];
        reap(&prev, &mut next, 999_999, ENDED_TTL_MS);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].ended_at, None);
    }

    #[test]
    fn clears_ended_at_if_a_session_comes_back_alive() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Working, None)];
        reap(&prev, &mut next, 6_000, ENDED_TTL_MS);
        assert_eq!(next[0].ended_at, None);
    }
}
