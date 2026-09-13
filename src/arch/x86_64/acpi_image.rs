//! Pure image finalization helpers, also executable as host-side regression tests.

pub fn finish_checksum(image: &mut [u8], offset: Option<usize>) {
    if let Some(offset) = offset {
        image[offset] = 0;
        image[offset] = 0u8.wrapping_sub(image.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)));
    }
}

/// Reserve an aligned table, returning its start (not the next cursor).
pub fn reserve_table(cursor: usize, len: usize, capacity: usize, alignment: usize) -> Option<usize> {
    if !alignment.is_power_of_two() {
        return None;
    }
    let start = cursor.checked_add(alignment - 1)? & !(alignment - 1);
    (start.checked_add(len)? <= capacity).then_some(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_and_truncation_checksum_final_image_only() {
        let source = [0x73u8; 132];
        let mut image = source;
        image[40..44].copy_from_slice(&0x12345678u32.to_le_bytes());
        image[40..44].copy_from_slice(&0x35300100u32.to_le_bytes());
        image[4..8].copy_from_slice(&100u32.to_le_bytes());
        finish_checksum(&mut image[..100], Some(9));
        assert_eq!(image[..100].iter().fold(0u8, |sum, b| sum.wrapping_add(*b)), 0);
        assert_eq!(source, [0x73; 132]);
        assert_eq!(image[100..], source[100..]);
        let once = image;
        finish_checksum(&mut image[..100], Some(9));
        assert_eq!(once, image);
    }

    #[test]
    fn facs_has_no_checksum() {
        let mut facs = [0xa5u8; 64];
        facs[..4].copy_from_slice(b"FACS");
        let original = facs;
        finish_checksum(&mut facs, None);
        assert_eq!(facs, original);
    }

    #[test]
    fn aligned_capacity_checked_without_overflow() {
        assert_eq!(reserve_table(37, 64, 128, 64), Some(64));
        assert_eq!(reserve_table(37, 65, 128, 64), None);
        assert_eq!(reserve_table(usize::MAX, 1, usize::MAX, 64), None);
        assert_eq!(reserve_table(0, usize::MAX, 1024, 8), None);
        assert_eq!(reserve_table(0, 1, 1024, 3), None);
    }
}
