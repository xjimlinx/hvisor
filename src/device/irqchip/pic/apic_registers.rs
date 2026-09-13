// Guest register state; never changes the host's APIC mode.
pub struct ApicRegisters {
    pub base: u64,
    pub regs: [u32; 64],
}
impl ApicRegisters {
    pub fn new(bsp: bool) -> Self {
        let mut regs = [0; 64];
        regs[0xf] = 0xff;
        regs[0xe] = u32::MAX;
        for index in [0x2f, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37] {
            regs[index] = 1 << 16;
        }
        Self { base: 0xfee00000 | 0xc00 | ((bsp as u64) << 8), regs }
    }
    pub fn set_base(&mut self, value: u64) -> bool {
        // Only the fixed guest LAPIC page is registered. Reject relocation,
        // reserved bits and the architecturally invalid enable=0,x2=1 state.
        if value & !0xd00 != 0xfee00000 || value & 0xc00 == 0x400 {
            return false;
        }
        // x2APIC -> xAPIC requires the intermediate disabled state.
        if self.base & 0xc00 == 0xc00 && value & 0xc00 == 0x800 {
            return false;
        }
        self.base = (value & !0x100) | (self.base & 0x100);
        true
    }
    pub fn index(offset: usize, size: usize) -> Option<usize> {
        if size == 4 && offset < 0x400 && offset & 15 == 0 {
            Some(offset >> 4)
        } else { None }
    }
    pub fn icr(&self, low: u32) -> u64 {
        ((self.regs[0x31] as u64 >> 24) << 32) | (low as u64 & !(1 << 12))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn mode_transition_preserves_bsp() {
        let mut a = ApicRegisters::new(true);
        assert!(!a.set_base(0xfee00800));
        assert!(a.set_base(0xfee00000));
        assert!(a.set_base(0xfee00800));
        assert_eq!(a.base, 0xfee00900);
        assert!(!a.set_base(0xfee01400));
        assert!(!a.set_base(0xfee01000));
    }
    #[test] fn access_width_and_icr() {
        assert_eq!(ApicRegisters::index(0xf0, 4), Some(15));
        assert_eq!(ApicRegisters::index(0xf1, 4), None);
        assert_eq!(ApicRegisters::index(0xf0, 8), None);
        let mut a = ApicRegisters::new(false);
        a.regs[0x31] = 0x12000000;
        assert_eq!(a.icr(0x1041), 0x1200000041);
        assert_eq!(a.regs[0x35], 1 << 16);
    }
}
