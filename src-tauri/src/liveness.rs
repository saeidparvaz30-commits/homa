use sysinfo::{Pid, System};

pub fn pid_alive(pid: u32, sys: &System) -> bool {
    sys.process(Pid::from_u32(pid)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_alive() {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
        assert!(pid_alive(std::process::id(), &sys));
    }

    #[test]
    fn absurd_pid_is_dead() {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
        assert!(!pid_alive(4_000_000_000, &sys));
    }
}
