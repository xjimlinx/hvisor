//! Z270 policy for INIT received by a hypervisor-owned parking VM.
//! Intel SDM vol. 3C, 25.2: INIT exits do not reset guest register state.
//! A parking VM is not an OS AP awaiting SIPI; leave its loop/state intact.
pub fn resume_parked_init(reason: u32, cpu: usize, root_mask: u64, running: bool) -> bool {
    reason == 3 && cpu > 0 && cpu < 64 && !running && root_mask & (1u64 << cpu) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_parked_cpus_resume() {
        for cpu in 1..8 { assert!(resume_parked_init(3, cpu, 1, false)); }
    }
    #[test]
    fn never_swallow_running_or_assigned_cpu_init() {
        assert!(!resume_parked_init(3, 0, 1, false));
        assert!(!resume_parked_init(3, 2, 1, true));
        assert!(!resume_parked_init(3, 2, 5, false));
        assert!(!resume_parked_init(3, 64, 1, false));
    }
    #[test]
    fn other_exits_remain_unhandled() {
        for reason in [0, 2, 4, 48, 49] {
            assert!(!resume_parked_init(reason, 2, 1, false));
        }
    }
}
