use crate::model::AgentStatus;

pub const WAITING_STATUSES: &[&str] =
    &["waiting", "waiting_for_input", "needs_input", "input", "blocked"];

pub fn map_status(raw: &str, pid_alive: bool) -> AgentStatus {
    if !pid_alive {
        return AgentStatus::Ended;
    }
    let r = raw.trim().to_ascii_lowercase();
    if r == "busy" {
        return AgentStatus::Working;
    }
    if r == "idle" {
        return AgentStatus::Idle;
    }
    if WAITING_STATUSES.contains(&r.as_str()) {
        return AgentStatus::Waiting;
    }
    // Unknown non-empty value: fail toward attention, never hide it.
    AgentStatus::Idle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentStatus;

    #[test]
    fn known_values_map() {
        assert_eq!(map_status("busy", true), AgentStatus::Working);
        assert_eq!(map_status("idle", true), AgentStatus::Idle);
    }

    #[test]
    fn waiting_set_maps_to_waiting() {
        assert_eq!(map_status("waiting", true), AgentStatus::Waiting);
        assert_eq!(map_status("waiting_for_input", true), AgentStatus::Waiting);
    }

    #[test]
    fn unknown_fails_toward_attention() {
        assert_eq!(map_status("banana", true), AgentStatus::Idle);
    }

    #[test]
    fn dead_pid_is_ended_regardless_of_status() {
        assert_eq!(map_status("busy", false), AgentStatus::Ended);
    }
}
