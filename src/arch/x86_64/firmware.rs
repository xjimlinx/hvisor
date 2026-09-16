//! Experimental Z270 firmware reset contract, not a PC/UEFI device model.
//! Backing must come from the existing Zone1 pool, never physical flash.
pub const BOOT_MODE: u32 = 2;
pub const RESET_VECTOR: u64 = 0xffff_fff0;
pub const CS_BASE: usize = 0xffff_0000;
pub const CS_SELECTOR: u16 = 0xf000;
pub const IP: usize = 0xfff0;
pub const POOL_START: u64 = 0x5f0000000;
pub const POOL_END: u64 = 0x870000000;

/// Tuples are (HPA, GPA, size). Only disjoint page-aligned RAM is accepted.
/// This intentionally disallows passthrough and aliases in the first probe.
pub fn valid_ram_layout(regions: &[(u64, u64, u64)]) -> bool {
    let mut reset = false;
    let mut low = false;
    for (i, &(hpa, gpa, size)) in regions.iter().enumerate() {
        let Some(hend) = hpa.checked_add(size) else { return false; };
        let Some(gend) = gpa.checked_add(size) else { return false; };
        if size == 0 || (hpa | gpa | size) & 4095 != 0 ||
            hpa < POOL_START || hend > POOL_END || gend > 0x1_0000_0000 {
            return false;
        }
        reset |= gpa <= RESET_VECTOR && gend >= RESET_VECTOR + 16;
        low |= gpa == 0 && size >= 0x10_0000;
        for &(h, g, n) in &regions[..i] {
            // Previous entries have already passed overflow checks.
            if (hpa < h+n && h < hend) || (gpa < g+n && g < gend) {
                return false;
            }
        }
    }
    reset && low
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reset_address() { assert_eq!(CS_BASE + IP, RESET_VECTOR as usize); }
    #[test] fn ram_only_rom_copy() {
        assert!(valid_ram_layout(&[(POOL_START, 0, 0x100000),
                                  (POOL_START+0x100000, 0xffc00000, 0x400000)]));
    }
    #[test] fn rejects_unsafe_layouts() {
        assert!(!valid_ram_layout(&[]));
        assert!(!valid_ram_layout(&[(POOL_START, 0, 0x100000)]));
        assert!(!valid_ram_layout(&[(0xffc00000, 0xffc00000, 0x400000)]));
        assert!(!valid_ram_layout(&[(POOL_START, 0, 0x100000),
                                   (POOL_START, 0xffc00000, 0x400000)]));
        assert!(!valid_ram_layout(&[(POOL_START, 0, 0x100000),
                                   (POOL_START+0x100000, 0x80000, 0x400000)]));
        assert!(!valid_ram_layout(&[(POOL_START, 0, u64::MAX)]));
        assert!(!valid_ram_layout(&[(POOL_START, 0, 0x100001)]));
        assert!(!valid_ram_layout(&[(POOL_START, 0, 0x100000),
                                   (POOL_END, 0xfffff000, 0x1000)]));
    }
}
