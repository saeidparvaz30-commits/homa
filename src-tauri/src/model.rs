use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Working,
    Idle,
    Waiting,
    Ended,
}

impl AgentStatus {
    pub fn priority(&self) -> u8 {
        match self {
            AgentStatus::Waiting => 3,
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
}
