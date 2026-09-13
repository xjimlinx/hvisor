//! HPET period is specified in femtoseconds, not an integer MHz frequency.
pub fn nanos_from_ticks(ticks: u64, period_fs: u64) -> u64 {
    ((ticks as u128 * period_fs as u128) / 1_000_000).min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn z270_24mhz_period_keeps_fractional_precision() {
        assert_eq!(nanos_from_ticks(24_000_000, 41_666_667), 1_000_000_008);
        assert_eq!(nanos_from_ticks(24_000, 41_666_667), 1_000_000);
    }
    #[test]
    fn long_uptime_does_not_overflow_intermediate() {
        assert_eq!(nanos_from_ticks(24_000_000 * 86400 * 365, 41_666_667), 31_536_000_252_288_000);
        assert_eq!(nanos_from_ticks(u64::MAX, 100_000_000), u64::MAX);
    }
}
