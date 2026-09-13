// 64-bit ModRM/SIB effective-address decoding. `tail_rip` points just after
// ModRM. Supported MOV forms have no trailing immediate.
pub fn decode(modrm: u8, rex: u8, tail: &[u8], tail_rip: u64,
              mut reg: impl FnMut(usize) -> u64) -> Option<(u64, usize)> {
    let mode = modrm >> 6;
    if mode == 3 { return None; }
    let rm = modrm & 7;
    let mut used = 0;
    let mut base;
    let mut rip_relative = false;
    let mut disp32 = mode == 2;
    if rm == 4 {
        let sib = *tail.get(used)?;
        used += 1;
        let index = (sib >> 3) & 7;
        base = if index == 4 && rex & 2 == 0 { 0 }
            else { reg(index as usize + if rex & 2 != 0 { 8 } else { 0 })
                .wrapping_shl((sib >> 6) as u32) };
        if mode == 0 && sib & 7 == 5 {
            disp32 = true;
        } else {
            base = base.wrapping_add(reg((sib & 7) as usize + if rex & 1 != 0 { 8 } else { 0 }));
        }
    } else if mode == 0 && rm == 5 {
        base = 0;
        disp32 = true;
        rip_relative = true;
    } else {
        base = reg(rm as usize + if rex & 1 != 0 { 8 } else { 0 });
    }
    if disp32 {
        let bytes: [u8; 4] = tail.get(used..used + 4)?.try_into().ok()?;
        used += 4;
        base = base.wrapping_add(i32::from_le_bytes(bytes) as i64 as u64);
    } else if mode == 1 {
        base = base.wrapping_add(*tail.get(used)? as i8 as i64 as u64);
        used += 1;
    }
    if rip_relative { base = base.wrapping_add(tail_rip.wrapping_add(used as u64)); }
    Some((base, used))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn real_arch_eoi_sib_absolute() {
        // native_apic_mem_eoi: 89 04 25 b0 d0 5f ff
        assert_eq!(decode(0x04, 0, &[0x25,0xb0,0xd0,0x5f,0xff], 0, |_| 0xdeadbeef),
                   Some((0xffffffffff5fd0b0, 5)));
    }
    #[test] fn sib_base_index_and_signed_disp() {
        assert_eq!(decode(0x44, 3, &[0x4c,0x80], 0, |r| r as u64 * 0x100),
                   Some((0xc00 + 2 * 0x900 - 128, 2)));
    }
    #[test] fn rip_relative_and_extended_base() {
        assert_eq!(decode(0x05, 0, &[0xfc,0xff,0xff,0xff], 0x1000, |_| 0), Some((0x1000,4)));
        assert_eq!(decode(0x00, 1, &[], 0, |r| r as u64), Some((8,0)));
        assert_eq!(decode(0x04, 0, &[0x25], 0, |_| 0), None);
    }
}
