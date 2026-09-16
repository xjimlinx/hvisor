//! Minimal ACPI-only PM interface for Z270 Zone1, never physical PCH access.
pub const EVENT: u16 = 0x1800;
pub const CONTROL: u16 = 0x1804;
pub const TIMER: u16 = 0x1808;
use core::sync::atomic::{AtomicU16, Ordering};
static EVENT_ENABLE: AtomicU16 = AtomicU16::new(0);

/// Guest-only enables; status stays clear until a virtual event is implemented.
pub fn write(port: u16, size: usize, value: u32) -> bool {
    if read(port, size, 0).is_none() { return false; }
    for byte in 0..size {
        let address = port as usize + byte;
        if address >= EVENT as usize + 2 && address < EVENT as usize + 4 {
            let shift = (address - EVENT as usize - 2) * 8;
            let mask = 0xffu16 << shift;
            let bits = (((value >> (byte * 8)) & 255) as u16) << shift;
            let _ = EVENT_ENABLE.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
                |old| Some((old & !mask) | bits));
        }
    }
    true
}

/// FADT legacy block contract (byte offsets from ACPI specification).
pub const DWORD_PATCHES: &[(usize, u32)] = &[
    (48, 0), // No SMI command: always in ACPI mode.
    (56, EVENT as u32), (60, 0),
    (64, CONTROL as u32), (68, 0),
    (72, 0), (76, TIMER as u32),
    (80, 0), (84, 0), // No host GPE blocks.
];
pub const BYTE_PATCHES: &[(usize, u8)] = &[
    (52, 0), (53, 0), (54, 0), (55, 0),
    (88, 4), (89, 2), (90, 0), (91, 4),
    (92, 0), (93, 0), (94, 0), (95, 0),
];

pub fn read(port: u16, size: usize, nanos: u64) -> Option<u32> {
    if !matches!(size, 1 | 2 | 4) { return None; }
    for (base, len, value) in [
        (EVENT, 4, (EVENT_ENABLE.load(Ordering::Relaxed) as u32) << 16),
        (CONTROL, 2, 1u32), // SCI_EN always set; SLP_EN never forwarded.
        (TIMER, 4, ((nanos as u128 * 3_579_545 / 1_000_000_000) as u32) & 0xff_ffff),
    ] {
        let offset = port.wrapping_sub(base) as usize;
        if offset.checked_add(size).is_some_and(|end| end <= len) {
            let mask = u32::MAX >> ((4 - size) * 8);
            return Some((value >> (offset * 8)) & mask);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn acpi_only_and_bounded_access() {
        assert_eq!(read(CONTROL, 2, 0), Some(1));
        assert_eq!(read(CONTROL + 1, 1, 0), Some(0));
        assert_eq!(read(CONTROL, 4, 0), None);
        assert_eq!(read(0xb2, 1, 0), None);
        assert!(write(EVENT + 2, 2, 0x120));
        assert_eq!(read(EVENT, 4, 0), Some(0x1200000));
        assert!(write(EVENT, 2, 0xffff));
        assert_eq!(read(EVENT, 2, 0), Some(0));
        assert!(write(EVENT + 2, 2, 0));
        assert_eq!(read(0xffff, 2, 0), None);
    }
    #[test] fn timer_frequency_and_wrap() {
        assert_eq!(read(TIMER, 4, 1_000_000_000), Some(3_579_545));
        assert_eq!(read(TIMER, 4, 10_000_000_000), Some(35_795_450 & 0xff_ffff));
        assert_eq!(read(TIMER + 1, 1, 1_000_000_000), Some((3_579_545 >> 8) & 255));
    }
}
