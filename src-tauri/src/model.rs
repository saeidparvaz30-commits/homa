use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Working,
    Idle,
    Waiting,
    Limited,
    Ended,
}

impl AgentStatus {
    pub fn priority(&self) -> u8 {
        match self {
            AgentStatus::Waiting => 4,
            AgentStatus::Limited => 3,
            AgentStatus::Idle => 2,
            AgentStatus::Working => 1,
            AgentStatus::Ended => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentState {
    pub pid: u32,
    pub session_id: String,
    pub name: String,
    pub cwd: String,
    pub repo: String,
    pub branch: Option<String>,
    pub status: AgentStatus,
    pub raw_status: String,
    pub started_at: i64,
    pub status_updated_at: i64,
    pub model: Option<String>,
    pub context_pct: Option<f32>,
    pub last_activity: Option<i64>,
    /// Wall clock ms when this session was first observed dead. Set by `reap`.
    pub ended_at: Option<i64>,
    /// Epoch ms when the usage limit lifts; None for credit/login limits.
    pub limited_until: Option<i64>,
    pub was_busy_at_limit: bool,
    pub resume_fired: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn priority_orders_waiting_above_idle_above_working_above_ended() {
        assert!(AgentStatus::Waiting.priority() > AgentStatus::Idle.priority());
        assert!(AgentStatus::Idle.priority() > AgentStatus::Working.priority());
        assert!(AgentStatus::Working.priority() > AgentStatus::Ended.priority());
    }

    #[test]
    fn limited_sits_between_waiting_and_idle() {
        assert!(AgentStatus::Waiting.priority() > AgentStatus::Limited.priority());
        assert!(AgentStatus::Limited.priority() > AgentStatus::Idle.priority());
    }
}
