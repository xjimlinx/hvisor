//! x2APIC clustered logical destination encoding.
pub const fn logical_id(apic_id: u32) -> u32 {
    ((apic_id >> 4) << 16) | (1 << (apic_id & 15))
}

pub const fn matches_logical(apic_id: u32, destination: u32) -> bool {
    let local = logical_id(apic_id);
    (local >> 16) == (destination >> 16) && (local & destination & 0xffff) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn z270_eight_threads_have_nonzero_distinct_logical_ids() {
        for id in 0..8 {
            assert_eq!(logical_id(id), 1 << id);
            assert!(matches_logical(id, 0xff));
            assert!(!matches_logical(id, 0));
        }
    }
    #[test]
    fn cluster_boundary() {
        assert_eq!(logical_id(16), 0x10001);
        assert!(!matches_logical(16, 1));
        assert!(!matches_logical(0, 0x10001));
        assert!(matches_logical(17, 0x10003));
    }
}
