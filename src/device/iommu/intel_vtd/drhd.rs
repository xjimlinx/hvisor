// SPDX-License-Identifier: MulanPSL-2.0
// Segment-zero endpoint routing. Unsupported hierarchy scopes fail closed.
use alloc::vec::Vec;

#[derive(Debug)]
pub struct Unit {
    pub base: usize,
    pub include_all: bool,
    pub endpoints: Vec<u16>,
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Unit>, &'static str> {
    if bytes.len() < 48 || &bytes[..4] != b"DMAR" {
        return Err("invalid DMAR header");
    }
    let u16_at = |i| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    if length != bytes.len() || bytes.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != 0 {
        return Err("invalid DMAR length/checksum");
    }
    let mut units: Vec<Unit> = Vec::new();
    let mut pos = 48;
    while pos < length {
        if pos + 4 > length { return Err("truncated DMAR entry"); }
        let kind = u16_at(pos);
        let size = u16_at(pos + 2) as usize;
        if size < 4 || size > length - pos { return Err("invalid DMAR entry length"); }
        if kind == 0 {
            if size < 16 || u16_at(pos + 6) != 0 { return Err("unsupported DRHD segment/header"); }
            let base = u64::from_le_bytes(bytes[pos+8..pos+16].try_into().unwrap()) as usize;
            if base == 0 || base & 4095 != 0 || units.iter().any(|u| u.base == base) {
                return Err("invalid/duplicate DRHD base");
            }
            let mut unit = Unit { base, include_all: bytes[pos+4] & 1 != 0, endpoints: Vec::new() };
            if unit.include_all && units.iter().any(|u| u.include_all) { return Err("multiple include-all units"); }
            let mut scope = pos + 16;
            while scope < pos + size {
                if scope + 6 > pos + size { return Err("truncated DRHD scope"); }
                let scope_size = bytes[scope+1] as usize;
                if scope_size < 8 || scope_size % 2 != 0 || scope_size > pos + size - scope {
                    return Err("invalid DRHD scope length");
                }
                match bytes[scope] {
                    1 => {
                        if scope_size != 8 { return Err("multi-hop endpoint scope requires topology resolver"); }
                        let dev = bytes[scope+6];
                        let function = bytes[scope+7];
                        if dev >= 32 || function >= 8 { return Err("invalid scope BDF"); }
                        let bdf = ((bytes[scope+5] as u16) << 8) | ((dev as u16) << 3) | function as u16;
                        if unit.endpoints.contains(&bdf) || units.iter().any(|u| u.endpoints.contains(&bdf)) {
                            return Err("ambiguous endpoint scope");
                        }
                        unit.endpoints.push(bdf);
                    }
                    3 | 4 => {} // IOAPIC/HPET scopes are not PCI DMA requesters.
                    _ => return Err("unsupported DRHD scope type"),
                }
                scope += scope_size;
            }
            units.push(unit);
        }
        pos += size;
    }
    if units.is_empty() { return Err("no DRHD units"); }
    Ok(units)
}

pub fn route(units: &[Unit], bdf: u16) -> Option<usize> {
    units.iter().position(|u| u.endpoints.contains(&bdf))
        .or_else(|| units.iter().position(|u| u.include_all))
}
