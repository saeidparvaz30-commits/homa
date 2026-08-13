use crate::model::{AgentState, AgentStatus};

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct TraySummary {
    pub top: AgentStatus,
    pub waiting: usize,
    pub limited: usize,
    pub idle: usize,
    pub working: usize,
}

pub fn summarize(agents: &[AgentState]) -> TraySummary {
    let mut top = AgentStatus::Ended;
    let (mut waiting, mut limited, mut idle, mut working) = (0, 0, 0, 0);
    for a in agents {
        if a.status.priority() > top.priority() {
            top = a.status;
        }
        match a.status {
            AgentStatus::Waiting => waiting += 1,
            AgentStatus::Limited => limited += 1,
            AgentStatus::Idle => idle += 1,
            AgentStatus::Working => working += 1,
            AgentStatus::Ended => {}
        }
    }
    TraySummary {
        top,
        waiting,
        limited,
        idle,
        working,
    }
}

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct Transition {
    pub session_id: String,
    pub name: String,
    pub to: AgentStatus,
}

pub fn transitions(prev: &[AgentState], next: &[AgentState]) -> Vec<Transition> {
    let mut out = Vec::new();
    for n in next {
        if !matches!(
            n.status,
            AgentStatus::Waiting | AgentStatus::Idle | AgentStatus::Limited
        ) {
            continue;
        }
        let old = prev.iter().find(|p| p.session_id == n.session_id);
        let changed = match old {
            Some(p) => p.status != n.status,
            None => true,
        };
        if changed {
            out.push(Transition {
                session_id: n.session_id.clone(),
                name: n.name.clone(),
                to: n.status,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentState, AgentStatus};

    fn a(session: &str, status: AgentStatus) -> AgentState {
        AgentState {
            pid: 1,
            session_id: session.into(),
            name: session.into(),
            cwd: "x".into(),
            repo: "x".into(),
            branch: None,
            status,
            raw_status: "".into(),
            started_at: 0,
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
    fn limited_counts_and_can_top_the_summary() {
        let s = summarize(&[a("1", AgentStatus::Working), a("2", AgentStatus::Limited)]);
        assert_eq!(s.top, AgentStatus::Limited);
        assert_eq!(s.limited, 1);
    }

    #[test]
    fn transition_into_limited_fires() {
        let prev = vec![a("1", AgentStatus::Working)];
        let next = vec![a("1", AgentStatus::Limited)];
        let t = transitions(&prev, &next);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].to, AgentStatus::Limited);
    }

    #[test]
    fn summary_top_is_highest_priority() {
        let s = summarize(&[
            a("1", AgentStatus::Working),
            a("2", AgentStatus::Idle),
            a("3", AgentStatus::Waiting),
        ]);
        assert_eq!(s.top, AgentStatus::Waiting);
        assert_eq!((s.waiting, s.idle, s.working), (1, 1, 1));
    }

    #[test]
    fn empty_summary_tops_ended() {
        assert_eq!(summarize(&[]).top, AgentStatus::Ended);
    }

    #[test]
    fn transition_fires_into_waiting_only_on_change() {
        let prev = vec![a("1", AgentStatus::Working)];
        let next = vec![a("1", AgentStatus::Waiting)];
        let t = transitions(&prev, &next);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].to, AgentStatus::Waiting);
    }

    #[test]
    fn no_transition_when_status_unchanged() {
        let prev = vec![a("1", AgentStatus::Idle)];
        let next = vec![a("1", AgentStatus::Idle)];
        assert!(transitions(&prev, &next).is_empty());
    }

    #[test]
    fn transition_into_working_does_not_fire() {
        let prev = vec![a("1", AgentStatus::Idle)];
        let next = vec![a("1", AgentStatus::Working)];
        assert!(transitions(&prev, &next).is_empty());
    }
}
