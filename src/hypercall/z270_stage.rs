//! Board-scoped, one-shot RAM-only Zone1 loader and polled UART log.
//! Loader never grants mappings. The board defines a trusted-backend CPU alias;
//! VT-d explicitly excludes it, and staging remains one-shot.
use crate::{config::{HvZoneConfig, MEM_TYPE_RAM}, cpu_data::this_zone,
    memory::MemFlags, zone::{find_zone, is_this_root_zone}, hypercall::HyperCallResult};
use spin::{Mutex, MutexGuard};

pub const START: usize = 0x5f0000000;
pub const END: usize = 0x870000000;
pub const MAGIC: usize = 0x5a314c01;
const PAGE: usize = 4096;
static SEALED: Mutex<bool> = Mutex::new(false);
const LOG_SIZE: usize = 128 * 1024;
struct Log { data: [u8; LOG_SIZE], head: usize, len: usize, lcr: u8, scratch: u8, ier: u8, mcr: u8, dll: u8, dlm: u8 }
static LOG: Mutex<Log> = Mutex::new(Log { data: [0; LOG_SIZE], head: 0, len: 0, lcr: 0, scratch: 0, ier: 0, mcr: 0, dll: 1, dlm: 0 });
static HOST_LOG: Mutex<Log> = Mutex::new(Log { data: [0; LOG_SIZE], head: 0, len: 0, lcr: 0, scratch: 0, ier: 0, mcr: 0, dll: 1, dlm: 0 });
pub fn host_log_byte(byte: u8) {
    let mut log = HOST_LOG.lock();
    let tail = (log.head + log.len) % LOG_SIZE;
    log.data[tail] = byte;
    if log.len == LOG_SIZE { log.head = (log.head + 1) % LOG_SIZE; }
    else { log.len += 1; }
}

pub fn target_page(addr: usize) -> bool {
    addr % PAGE == 0 && addr >= START && addr.checked_add(PAGE).is_some_and(|e| e <= END)
}

fn root_page(gpa: usize, write: bool) -> HyperCallResult {
    if gpa % PAGE != 0 { return hv_result_err!(EINVAL); }
    let root = this_zone();
    let inner = root.read();
    let (hpa, flags, _) = match unsafe { inner.gpm().page_table_query(gpa) } {
        Ok(v) => v, Err(_) => return hv_result_err!(EFAULT),
    };
    if !flags.contains(if write { MemFlags::WRITE } else { MemFlags::READ }) || flags.contains(MemFlags::IO) {
        return hv_result_err!(EPERM);
    }
    // Static root RAM allowlist also excludes framebuffer and other dynamic mappings.
    if !crate::platform::ROOT_ZONE_MEMORY_REGIONS.iter().any(|r|
        r.mem_type == MEM_TYPE_RAM && hpa >= r.physical_start as usize &&
        hpa.checked_add(PAGE).is_some_and(|end| end <= (r.physical_start + r.size) as usize)) {
        return hv_result_err!(EPERM);
    }
    Ok(hpa)
}

pub fn dispatch(code: u64, arg0: usize, arg1: usize) -> HyperCallResult {
    if !is_this_root_zone() { return hv_result_err!(EPERM); }
    if code == 12 { return if arg0 == 0 && arg1 == 0 { Ok(MAGIC) } else { hv_result_err!(EINVAL) }; }
    if code == 13 {
        let sealed = SEALED.lock();
        if *sealed || find_zone(1).is_some() { return hv_result_err!(EBUSY); }
        if !target_page(arg1) { return hv_result_err!(EINVAL); }
        let source = root_page(arg0, false)?;
        unsafe { core::ptr::copy_nonoverlapping(source as *const u8, arg1 as *mut u8, PAGE); }
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        return Ok(0);
    }
    if arg1 != PAGE { return hv_result_err!(EINVAL); }
    let dest = root_page(arg0, true)?;
    if code == 15 { igd_snapshot(); }
    let mut log = if code == 15 { HOST_LOG.lock() } else { LOG.lock() };
    let count = log.len.min(PAGE);
    for i in 0..count {
        unsafe { *((dest + i) as *mut u8) = log.data[log.head]; }
        log.head = (log.head + 1) % LOG_SIZE;
    }
    log.len -= count;
    Ok(count)
}

/// Root-requested, rate-limited, read-only snapshot; no arbitrary MMIO address API.
fn igd_snapshot() {
    use core::{ptr::read_volatile, sync::atomic::{AtomicU64, Ordering}};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = crate::arch::hpet::current_time_nanos();
    let old = LAST.load(Ordering::Relaxed);
    if now.wrapping_sub(old) < 10_000_000_000 ||
        LAST.compare_exchange(old, now, Ordering::Relaxed, Ordering::Relaxed).is_err() { return; }
    let Some(zone) = find_zone(1) else { return; };
    let inner = zone.read();
    if !matches!(unsafe { inner.gpm().page_table_query(0xdd000000) },
        Ok((0xdd000000, flags, _)) if flags.contains(MemFlags::READ)) { return; }
    let cfg = 0xe0010000usize;
    unsafe {
        if read_volatile(cfg as *const u32) != 0x59128086 ||
            read_volatile((cfg + 0x10) as *const u32) & !15 != 0xdd000000 { return; }
        info!("IGD SNAP ATS={:#x} PASID={:#x} BDSM={:#x} ASLS={:#x}",
            read_volatile((cfg+0x206) as *const u16),
            read_volatile((cfg+0x106) as *const u16),
            read_volatile((cfg+0x5c) as *const u32),
            read_volatile((cfg+0xfc) as *const u32));
        // Current validated plane: GGTT c0000, 1366x768 XR24, stride 5504.
        let mut invalid = 0;
        let mut unmapped = 0;
        for index in 0xc0usize..0x4c8 {
            let pte = read_volatile((0xdd800000 + index*8) as *const u64);
            if pte & 1 == 0 { invalid += 1; continue; }
            let iova = (pte & 0x000ffffffffff000) as usize;
            let mapping = inner.gpm().page_table_query(iova);
            if mapping.is_err() { unmapped += 1; }
            if index == 0xc0 || index == 0x4c7 {
                info!("IGD SNAP GGTT[{:x}]={:#x} EPT={:?}", index, pte, mapping);
            }
        }
        info!("IGD SNAP scanout pages=1032 invalid={} unmapped={}", invalid, unmapped);
    }
}

pub fn seal_for_start(config: &HvZoneConfig) -> crate::error::HvResult<MutexGuard<'static, bool>> {
    let mut sealed = SEALED.lock();
    let asmedia = config.num_pci_devs == 4 && {
        let d = config.alloc_pci_devs[3];
        d.domain == 0 && d.bus == 4 && d.device == 0 && d.function == 0 &&
            d.v_bus == 0 && d.v_device == 0x1c && d.v_function == 0 &&
            d.dev_type == crate::pci::vpci_dev::VpciDevType::Physical
    };
    let igpu = (config.num_pci_devs == 2 || config.num_pci_devs == 3 || asmedia) && config.num_pci_bus == 1 && {
        let bridge = config.alloc_pci_devs[0];
        let d = config.alloc_pci_devs[1];
        bridge.domain == 0 && bridge.bus == 0 && bridge.device == 0 && bridge.function == 0 &&
            bridge.v_bus == 0 && bridge.v_device == 0 && bridge.v_function == 0 &&
        d.domain == 0 && d.bus == 0 && d.device == 2 && d.function == 0 &&
            d.dev_type == crate::pci::vpci_dev::VpciDevType::Physical &&
            d.v_bus == 0 && d.v_device == 2 && d.v_function == 0 &&
        (config.num_pci_devs == 2 || {
            let pch = config.alloc_pci_devs[2];
            // 00:1f.0 is host-owned physically. Look up the identity-only
            // stub through the otherwise empty 00:1e.0 slot, but present it
            // to the guest at standard 00:1f.0 so ACPI and PCI agree.
            pch.domain == 0 && pch.bus == 0 && pch.device == 0x1e && pch.function == 0 &&
                pch.v_bus == 0 && pch.v_device == 0x1f && pch.v_function == 0 &&
                pch.dev_type == crate::pci::vpci_dev::VpciDevType::PchStub
        })
    };
    if *sealed || config.zone_id != 1 || config.cpus() != alloc::vec![2,3,6,7] ||
        (config.num_pci_devs != 0 && !igpu) {
        return hv_result_err!(EINVAL);
    }
    for r in config.memory_regions() {
        if r.size == 0 || (r.mem_type == MEM_TYPE_RAM &&
            (r.physical_start < START as u64 ||
             r.physical_start.checked_add(r.size).map_or(true, |end| end > END as u64))) {
            return hv_result_err!(EINVAL);
        }
        if r.mem_type != MEM_TYPE_RAM {
            if r.mem_type == crate::config::MEM_TYPE_VIRTIO &&
                (r.physical_start == 0x90000000 || r.physical_start == 0x90001000) &&
                r.virtual_start == r.physical_start &&
                r.size == 0x1000 && igpu {
                continue;
            }
            if !igpu || r.mem_type != crate::config::MEM_TYPE_IO { return hv_result_err!(EINVAL); }
            let valid = [(0xdd000000u64,0x1000000u64),(0xb0000000,0x10000000),
                         (0xf000,0x1000),(0x7a792000,0x3000),(0x7c000000,0x4000000),
                         // Full firmware IGD RMRR, including the 8 MiB before BDSM.
                         // Keep the old subrange accepted for rollback configurations.
                         (0x7b800000,0x4800000)];
            let usb_bar = asmedia && r.physical_start == 0xdf200000 &&
                r.virtual_start == 0xdf200000 && r.size == 0x8000;
            if !usb_bar && !valid.iter().any(|(s,n)| r.physical_start == *s && r.virtual_start == *s && r.size == *n) {
                return hv_result_err!(EINVAL);
            }
        }
    }
    if asmedia && !config.memory_regions().iter().any(|r|
        r.mem_type == crate::config::MEM_TYPE_IO && r.physical_start == 0xdf200000 &&
        r.virtual_start == 0xdf200000 && r.size == 0x8000) {
        return hv_result_err!(EINVAL);
    }
    // Fail closed even if zone_create fails part-way; a host reboot is needed to retry.
    *sealed = true;
    Ok(sealed)
}

// Minimal polled 16550 console, isolated from Zone0's UART state.
pub fn uart_write(port: u16, value: u32) {
    let mut log = LOG.lock();
    match port - 0x3f8 {
        0 if log.lcr & 0x80 == 0 => {
            let tail = (log.head + log.len) % LOG_SIZE;
            log.data[tail] = value as u8;
            if log.len == LOG_SIZE { log.head = (log.head + 1) % LOG_SIZE; }
            else { log.len += 1; }
        },
        3 => log.lcr = value as u8,
        0 => log.dll = value as u8,
        1 if log.lcr & 0x80 != 0 => log.dlm = value as u8,
        1 => log.ier = value as u8 & 0x0f,
        4 => log.mcr = value as u8,
        7 => log.scratch = value as u8,
        _ => {},
    }
}
pub fn uart_read(port: u16) -> u32 {
    let log = LOG.lock();
    match port - 0x3f8 {
        0 if log.lcr & 0x80 != 0 => log.dll as u32,
        1 if log.lcr & 0x80 != 0 => log.dlm as u32,
        1 => log.ier as u32, 2 => 0xc1, 3 => log.lcr as u32, 4 => log.mcr as u32,
        5 => 0x60,
        6 if log.mcr & 0x10 != 0 => ((log.mcr & 1) << 5 | (log.mcr & 2) << 3 | (log.mcr & 4) << 4 | (log.mcr & 8) << 4) as u32,
        6 => 0xb0, 7 => log.scratch as u32, _ => 0,
    }
}
