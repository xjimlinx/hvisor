// Copyright (c) 2025 Syswonder
// hvisor is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//     http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND, EITHER
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT, MERCHANTABILITY OR
// FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.
//
// Syswonder Website:
//      https://www.syswonder.org
//
// Authors:
//  Solicey <lzoi_lth@163.com>

use crate::{
    arch::{acpi, hpet::current_time_nanos},
    memory::{Frame, HostPhysAddr},
    zone::this_zone_id,
};
use ::acpi::sdt::Signature;
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use bit_field::BitField;
use core::{
    arch::asm,
    hint::spin_loop,
    mem::size_of,
    ptr::{read_volatile, write_volatile},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    usize,
};
use dma_remap_reg::*;
use spin::{Mutex, Once};
use x86_64::instructions::port::Port;

const IR_ENTRY_CNT: usize = 256;

const ROOT_TABLE_ENTRY_SIZE: usize = 16;
const CONTEXT_TABLE_ENTRY_SIZE: usize = 16;

const INVALIDATION_QUEUE_SIZE: usize = 4096;
const QI_INV_ENTRY_SIZE: usize = 16;
const NUM_IR_ENTRIES_PER_PAGE: usize = 256;

const INV_CONTEXT_CACHE_DESC: u64 = 0x01;
const INV_IOTLB_DESC: u64 = 0x02;
const INV_WAIT_DESC: u64 = 0x05;

const INV_STATUS_WRITE: u64 = 1 << 5;
const INV_STATUS_INCOMPLETED: u64 = 0;
const INV_STATUS_COMPLETED: u64 = 1;
const INV_STATUS_DATA: u64 = INV_STATUS_COMPLETED << 32;
const INV_WAIT_DESC_LOWER: u64 = INV_WAIT_DESC | INV_STATUS_WRITE | INV_STATUS_DATA;

const DMA_CONTEXT_DEVICE_INVL: u64 = (3 << 4);

const DMA_IOTLB_DOMAIN_INVL: u64 = (2 << 4);
const DMA_IOTLB_DW: u64 = (1 << 6);
const DMA_IOTLB_DR: u64 = (1 << 7);

//  DMA-remapping registers

mod dma_remap_reg {
    /// Capability Register
    pub(super) const DMAR_CAP_REG: usize = 0x8;
    /// Extended Capability Register
    pub(super) const DMAR_ECAP_REG: usize = 0x10;
    /// Global Command Register
    pub(super) const DMAR_GCMD_REG: usize = 0x18;
    /// Global Status Register
    pub(super) const DMAR_GSTS_REG: usize = 0x1c;
    /// Root Table Address Register
    pub(super) const DMAR_RTADDR_REG: usize = 0x20;
    /// Fault Status Register
    pub(super) const DMAR_FSTS_REG: usize = 0x34;
    /// Fault Event Control Register
    pub(super) const DMAR_FECTL_REG: usize = 0x38;
    /// Invalidation Queue Head Register
    pub(super) const DMAR_IQH_REG: usize = 0x80;
    /// Invalidation Queue Tail Register
    pub(super) const DMAR_IQT_REG: usize = 0x88;
    /// Invalidation Queue Address Register
    pub(super) const DMAR_IQA_REG: usize = 0x90;
    /// Interrupt Remapping Table Address Register
    pub(super) const DMAR_IRTA_REG: usize = 0xb8;
}

static VTD: Once<Mutex<Vtd>> = Once::new();
static LAST_FAULT_CHECK_NS: AtomicU64 = AtomicU64::new(0);
static FAULT_REPORT_COUNT: AtomicUsize = AtomicUsize::new(0);
const MAX_FAULT_REPORTS: usize = 16;
const PCI_DMA_DRAIN_NS: u64 = 100_000_000;

const PCI_COMMAND_OFFSET: usize = 0x04;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;
const PCI_COMMAND_INTX_DISABLE: u16 = 1 << 10;
const PCI_STATUS_OFFSET: usize = 0x06;
const PCI_STATUS_CAPABILITIES_LIST: u16 = 1 << 4;
const PCI_CLASS_REVISION_OFFSET: usize = 0x08;
const PCI_HEADER_TYPE_OFFSET: usize = 0x0e;
const PCI_BAR0_OFFSET: usize = 0x10;
const PCI_CAPABILITIES_PTR_OFFSET: usize = 0x34;
const PCI_SECONDARY_BUS_OFFSET: usize = 0x19;
const PCI_CAP_ID_MSI: u8 = 0x05;
const PCI_CAP_ID_MSIX: u8 = 0x11;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug)]
    struct EcapFlags: u64 {
        ///  Extended Interrupt Mode
        const EIM = 1 << 4;
        ///  Interrupt Remapping Support
        const IR = 1 << 3;
        ///  Queued Invalidation Support
        const QI = 1 << 1;
    }

    #[derive(Clone, Copy, Debug)]
    struct GstsFlags: u32 {
        /// Translation Enable Status
        const TES = 1 << 31;
        /// Root Table Pointer Status
        const RTPS = 1 << 30;
        /// Queue Invalidation Enable Status
        const QIES = 1 << 26;
        /// Interrupt Remapping Enable Status
        const IRES = 1 << 25;
        /// Interrupt Remap Table Pointer Status
        const IRTPS = 1 << 24;
    }

    #[derive(Clone, Copy, Debug)]
    struct GcmdFlags: u32 {
        /// Translation Enable
        const TE = 1 << 31;
        /// Set Root Table Pointer
        const SRTP = 1 << 30;
        /// Queue Invalidation Enable
        const QIE = 1 << 26;
        /// Interrupt Remapping Enable
        const IRE = 1 << 25;
        /// Set Interrupt Remap Table Pointer
        const SIRTP = 1 << 24;
    }
}

/*numeric_enum_macro::numeric_enum! {
#[repr(u8)]
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceScopeType {
    NotUsed = 0x00,
    PciEndpointDevice = 0x01,
    PciSubHierarchy = 0x02,
    IoApic = 0x03,
    MsiCapableHpet = 0x04,
    AcpiNamespaceDevice = 0x05
}
}*/

#[derive(Clone, Debug)]
struct VtdDevice {
    zone_id: usize,
    bus: u8,
    dev_func: u8,
}

#[derive(Clone, Debug)]
#[repr(C)]
struct DmarEntry {
    lo_64: u64,
    hi_64: u64,
}

#[derive(Debug)]
struct Vtd {
    reg_base_hpa: usize,
    devices: BTreeMap<u64, usize>,

    root_table: Frame,
    context_tables: BTreeMap<u8, Frame>,
    qi_queue: Frame,
    // Device-written completion storage must outlive every submission,
    // including a timed-out request. Do not use a stack-local DMA target.
    qi_completion: Frame,
    ir_table: Frame,
    /// cache value of DMAR_GCMD_REG
    gcmd: GcmdFlags,
    qi_queue_hpa: usize,
    qi_tail: usize,
}

impl Vtd {
    fn activate(&mut self) {
        self.quiesce_unassigned_pci_endpoints();
        self.wait_for_pci_dma_drain();
        // A request issued before Bus Master was cleared may finish during the
        // drain interval.  Start the actual translation test with clean fault
        // records so it cannot hide a later AHCI fault.
        self.clear_stale_faults();
        self.activate_dma_translation();
    }

    fn activate_dma_translation(&mut self) {
        if !self.gcmd.contains(GcmdFlags::TE) {
            self.gcmd |= GcmdFlags::TE;
            self.mmio_write_u32(DMAR_GCMD_REG, self.gcmd.bits());

            self.wait(GstsFlags::TES, false);
        }
    }

    fn activate_interrupt_remapping(&mut self) {
        if !self.gcmd.contains(GcmdFlags::IRE) {
            self.gcmd |= GcmdFlags::IRE;
            self.mmio_write_u32(DMAR_GCMD_REG, self.gcmd.bits());

            self.wait(GstsFlags::IRES, false);
        }
    }

    fn activate_qi(&mut self) {
        self.qi_queue_hpa = self.qi_queue.start_paddr();
        assert_eq!(size_of::<DmarEntry>(), QI_INV_ENTRY_SIZE);
        flush_cache_range(self.qi_queue_hpa, INVALIDATION_QUEUE_SIZE);
        self.mmio_write_u64(DMAR_IQA_REG, self.qi_queue_hpa as u64);
        self.mmio_write_u32(DMAR_IQT_REG, 0);

        if !self.gcmd.contains(GcmdFlags::QIE) {
            self.gcmd |= GcmdFlags::QIE;

            self.mmio_write_u32(DMAR_GCMD_REG, self.gcmd.bits());

            self.wait(GstsFlags::QIES, false);
        }
    }

    fn update_context_entry(
        &mut self,
        bus: u8,
        dev_func: u8,
        zone_s2pt_hpa: HostPhysAddr,
        is_insert: bool,
    ) {
        let root_entry_hpa = self.root_table.start_paddr() + (bus as usize) * ROOT_TABLE_ENTRY_SIZE;
        let root_entry_low = unsafe { &mut *(root_entry_hpa as *mut u64) };
        let zone_id = this_zone_id();

        // context table not present
        if !root_entry_low.get_bit(0) {
            let context_table = Frame::new_zero().unwrap();
            let context_table_hpa = context_table.start_paddr();

            // Publish zero/non-present entries too, before publishing the
            // parent pointer to a non-coherent remapping unit.
            flush_cache_range(context_table_hpa, 4096);

            // set context-table pointer
            root_entry_low.set_bits(12..=63, context_table_hpa.get_bits(12..=63) as _);
            // set present
            root_entry_low.set_bit(0, true);

            flush_cache_range(root_entry_hpa, ROOT_TABLE_ENTRY_SIZE);
            self.context_tables.insert(bus, context_table);
        }

        let context_table_hpa = self.context_tables.get(&bus).unwrap().start_paddr();
        let context_entry_hpa = context_table_hpa + (dev_func as usize) * CONTEXT_TABLE_ENTRY_SIZE;
        let context_entry = unsafe { &mut *(context_entry_hpa as *mut u128) };

        if is_insert {
            // address width: 010b (48bit 4-level page table)
            context_entry.set_bits(64..=66, 0b010);
            // domain identifier: zone id
            context_entry.set_bits(72..=87, zone_id as _);
            // second stage page translation pointer
            context_entry.set_bits(12..=63, zone_s2pt_hpa.get_bits(12..=63) as _);
            // present
            context_entry.set_bit(0, true);
            info!(
                "VT-d context {:02x}:{:02x}.{}: domain={}, s2pt={:#x}",
                bus,
                dev_func >> 3,
                dev_func & 0x7,
                zone_id,
                zone_s2pt_hpa
            );
        } else {
            context_entry.set_bits(0..=127, 0);
        }

        flush_cache_range(context_entry_hpa, CONTEXT_TABLE_ENTRY_SIZE);
        let bdf: u16 = (bus as u16) << 8 | (dev_func as u16);
        self.invalidate_context_cache(zone_id as _, bdf as _, 0);
    }

    fn add_device(&mut self, zone_id: usize, bdf: u64) {
        self.devices.insert(bdf, zone_id);
    }

    fn add_interrupt_table_entry(&mut self, irq: u32) {
        assert!(irq < (IR_ENTRY_CNT as u32));

        let ir_table_hpa = self.ir_table.start_paddr();
        let irte_hpa = ir_table_hpa + (irq as usize) * size_of::<u128>();
        let irte_ptr = irte_hpa as *mut u128;
        let mut irte: u128 = 0;

        // present
        irte.set_bit(0, true);
        // irte mode: remap
        irte.set_bit(15, false);
        // vector
        irte.set_bits(16..=23, irq as _);
        // dest id
        irte.set_bits(32..=63, 0);

        unsafe { *irte_ptr = irte };
        flush_cache_range(irte_hpa, size_of::<u128>());

        // TODO: iec
    }

    fn check_capability(&mut self) {
        let cap = self.mmio_read_u64(DMAR_CAP_REG);
        let ecap = self.mmio_read_u64(DMAR_ECAP_REG);
        info!("cap: {:x?} ecap: {:x?}", cap, ecap);
        assert!(EcapFlags::from_bits_truncate(ecap)
            .contains(EcapFlags::EIM | EcapFlags::IR | EcapFlags::QI));
    }

    fn clear_devices(&mut self, zone_id: usize) {
        let bdfs: Vec<(u8, u8)> = self
            .devices
            .iter()
            .filter(|&(_, &dev_zone_id)| dev_zone_id == zone_id)
            .map(|(&bdf, _)| (bdf.get_bits(8..=15) as u8, bdf.get_bits(0..=7) as u8))
            .collect();

        for (bus, dev_func) in bdfs {
            self.update_context_entry(bus, dev_func, 0, false);
        }
        self.invalid_iotlb(zone_id as _);
    }

    fn flush(&mut self, zone_id: usize, bus: u8, dev_func: u8) {
        let bdf: u16 = (bus as u16) << 8 | (dev_func as u16);
        self.invalidate_context_cache(zone_id as _, bdf as _, 0);
        self.invalid_iotlb(zone_id as _);
    }

    fn init(&mut self) {
        self.check_capability();
        self.mask_fault_interrupt();
        self.clear_stale_faults();
        self.set_root_table();
        self.activate_qi();

        /* self.set_interrupt_remap_table();
        for irq in 0..IR_ENTRY_CNT {
            self.add_interrupt_table_entry(irq as _);
        }
        self.activate_interrupt_remapping(); */
    }

    fn invalidate_context_cache(&mut self, domain_id: u16, source_id: u16, func_mask: u8) {
        let entry: DmarEntry = DmarEntry {
            lo_64: INV_CONTEXT_CACHE_DESC
                | DMA_CONTEXT_DEVICE_INVL
                | dma_ccmd_did(domain_id)
                | dma_ccmd_sid(source_id)
                | dma_ccmd_fm(func_mask),
            hi_64: 0,
        };
        if (entry.lo_64 != 0) {
            self.issue_qi_request(entry);
        }
    }

    fn invalid_iotlb(&mut self, domain_id: u16) {
        let entry: DmarEntry = DmarEntry {
            // drain read & drain write
            lo_64: INV_IOTLB_DESC
                | DMA_IOTLB_DOMAIN_INVL
                | DMA_IOTLB_DR
                | DMA_IOTLB_DW
                | dma_iotlb_did(domain_id),
            hi_64: 0,
        };
        if (entry.lo_64 != 0) {
            self.issue_qi_request(entry);
        }
    }

    fn issue_qi_request(&mut self, entry: DmarEntry) {
        // Vtd is locked by the caller; each submission waits for completion,
        // so at most two descriptors are outstanding in the 256-entry ring.
        let status_hpa = self.qi_completion.start_paddr();
        let status_ptr = status_hpa as *mut u32;
        unsafe { write_volatile(status_ptr, INV_STATUS_INCOMPLETED as u32) };
        flush_cache_range(status_hpa, size_of::<u32>());
        let first = self.qi_tail;
        unsafe {
            write_volatile((self.qi_queue_hpa + first) as *mut DmarEntry, entry);
        }
        self.qi_tail = (self.qi_tail + QI_INV_ENTRY_SIZE) % INVALIDATION_QUEUE_SIZE;
        let second = self.qi_tail;
        unsafe {
            write_volatile((self.qi_queue_hpa + second) as *mut DmarEntry, DmarEntry {
                lo_64: INV_WAIT_DESC_LOWER,
                hi_64: status_hpa as u64,
            });
        }
        self.qi_tail = (self.qi_tail + QI_INV_ENTRY_SIZE) % INVALIDATION_QUEUE_SIZE;
        // Flush separately: the pair can wrap at the end of the ring.
        flush_cache_range(self.qi_queue_hpa + first, QI_INV_ENTRY_SIZE);
        flush_cache_range(self.qi_queue_hpa + second, QI_INV_ENTRY_SIZE);
        self.mmio_write_u32(DMAR_IQT_REG, self.qi_tail as _);

        let start_tick = current_time_nanos();
        loop {
            // A dedicated frame prevents invalidating unrelated live data.
            flush_cache_range(status_hpa, size_of::<u32>());
            if unsafe { read_volatile(status_ptr) } == INV_STATUS_COMPLETED as u32 {
                break;
            }
            let faults = self.mmio_read_u32(DMAR_FSTS_REG);
            assert_eq!(faults & ((1 << 4) | (1 << 5) | (1 << 6)), 0,
                "VT-d queued invalidation error: FSTS={:#x}", faults);
            assert!(current_time_nanos().wrapping_sub(start_tick) <= 1_000_000_000,
                "VT-d queued invalidation timeout: head={:#x} tail={:#x} FSTS={:#x}",
                self.mmio_read_u64(DMAR_IQH_REG), self.qi_tail, faults);
            spin_loop();
        }
    }

    fn mask_fault_interrupt(&mut self) {
        // No VT-d fault-event vector/handler is installed yet.  Leaving the
        // event unmasked with FEADDR/FEDATA unset can route faults to an
        // undefined destination.  Poll FSTS/FRCD from VM-exit instead.
        self.mmio_write_u32(DMAR_FECTL_REG, 1 << 31);
    }

    fn clear_stale_faults(&self) {
        let fsts = self.mmio_read_u32(DMAR_FSTS_REG);
        if fsts == 0 {
            return;
        }

        let cap = self.mmio_read_u64(DMAR_CAP_REG);
        let fault_record_offset = (((cap >> 24) & 0x3ff) as usize) * 16;
        let fault_record_count = (((cap >> 40) & 0xff) as usize) + 1;
        warn!("clearing stale VT-d fault state: fsts={:#010x}", fsts);
        for index in 0..fault_record_count {
            let high_offset = fault_record_offset + index * 16 + 8;
            if self.mmio_read_u64(high_offset).get_bit(63) {
                // FRCD.F is the high dword's write-one-to-clear bit 31.
                self.mmio_write_u64(high_offset, 1 << 63);
            }
        }
        // Clear the write-one-to-clear status bits; read-only bits are ignored.
        self.mmio_write_u32(DMAR_FSTS_REG, fsts);
    }

    fn set_interrupt_remap_table(&mut self) {
        // bit 12-63: ir table address
        // bit 11: x2apic mode active
        // bit 0-3: X, where 2 ^ (X + 1) == number of entries
        let address: u64 =
            (self.ir_table.start_paddr() as u64) | (1 << 11) | ((IR_ENTRY_CNT.ilog2() - 1) as u64);

        self.mmio_write_u64(DMAR_IRTA_REG, address);
        self.mmio_write_u32(DMAR_GCMD_REG, (self.gcmd | GcmdFlags::SIRTP).bits());

        self.wait(GstsFlags::IRTPS, false);
    }

    fn set_root_table(&mut self) {
        flush_cache_range(self.root_table.start_paddr(), 4096);
        self.mmio_write_u64(DMAR_RTADDR_REG, self.root_table.start_paddr() as _);
        self.mmio_write_u32(DMAR_GCMD_REG, (self.gcmd | GcmdFlags::SRTP).bits());

        self.wait(GstsFlags::RTPS, false);
    }

    fn fill_dma_translation_tables(&mut self, zone_id: usize, zone_s2pt_hpa: HostPhysAddr) {
        let ecap = self.mmio_read_u64(DMAR_ECAP_REG);
        if !ecap.get_bit(0) {
            // ECAP.C=0 means DMA-remapping page-table walks are not cache
            // coherent.  Zone0's EPT hierarchy has already been built with
            // normal cached CPU stores, so make the entire hierarchy visible
            // before publishing its root in context entries.  This broad
            // one-shot synchronization is intentional for bring-up; later
            // map/unmap updates still need range-specific writeback.
            info!("VT-d ECAP.C=0: writing back cached DMA page tables");
            unsafe { asm!("wbinvd", options(nostack, preserves_flags)) };
        }

        let bdfs: Vec<(u8, u8)> = self
            .devices
            .iter()
            .filter(|&(_, &dev_zone_id)| dev_zone_id == zone_id)
            .map(|(&bdf, _)| (bdf.get_bits(8..=15) as u8, bdf.get_bits(0..=7) as u8))
            .collect();

        for (bus, dev_func) in bdfs {
            self.update_context_entry(bus, dev_func, zone_s2pt_hpa, true);
        }
        self.invalid_iotlb(zone_id as _);
    }

    fn quiesce_unassigned_pci_endpoints(&mut self) {
        // Walk every reachable bus from each ECAM root.  Merely hiding a PCI
        // function from Zone0 does not stop DMA left active by firmware.
        for root in crate::platform::ROOT_PCI_CONFIG.iter() {
            if root.ecam_base == 0 {
                continue;
            }

            let mut pending = Vec::new();
            let mut seen = [false; 256];
            pending.push(root.bus_range_begin as u8);

            while let Some(bus) = pending.pop() {
                if seen[bus as usize] {
                    continue;
                }
                seen[bus as usize] = true;

                for device in 0u8..32 {
                    let function0 = pci_ecam_function(root.ecam_base as usize, bus, device, 0);
                    if unsafe { read_volatile(function0 as *const u16) } == 0xffff {
                        continue;
                    }
                    let header_type =
                        unsafe { read_volatile((function0 + PCI_HEADER_TYPE_OFFSET) as *const u8) };
                    let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };

                    for function in 0u8..function_count {
                        let config =
                            pci_ecam_function(root.ecam_base as usize, bus, device, function);
                        if unsafe { read_volatile(config as *const u16) } == 0xffff {
                            continue;
                        }

                        let class_revision = unsafe {
                            read_volatile((config + PCI_CLASS_REVISION_OFFSET) as *const u32)
                        };
                        let base_class = (class_revision >> 24) as u8;
                        let sub_class = (class_revision >> 16) as u8;
                        let prog_if = (class_revision >> 8) as u8;
                        if base_class == 0x06 && sub_class == 0x04 {
                            let secondary = unsafe {
                                read_volatile((config + PCI_SECONDARY_BUS_OFFSET) as *const u8)
                            };
                            if secondary != 0 && !seen[secondary as usize] {
                                pending.push(secondary);
                            }
                            continue;
                        }

                        let bdf = ((bus as u64) << 8) | ((device as u64) << 3) | function as u64;
                        if self.devices.contains_key(&bdf) {
                            #[cfg(z270_minimal_acpi)]
                            if bus == 5 && device == 0 && function == 0 {
                                // AX210 must not retain firmware DMA/IRQ when
                                // entering the translated Zone0 address space.
                                self.mask_pci_interrupts(config);
                                let ptr = (config + PCI_COMMAND_OFFSET) as *mut u16;
                                unsafe {
                                    let old = read_volatile(ptr);
                                    write_volatile(ptr, (old & !PCI_COMMAND_BUS_MASTER) | PCI_COMMAND_INTX_DISABLE);
                                    asm!("mfence", options(nostack, preserves_flags));
                                    assert_eq!(read_volatile(ptr) & PCI_COMMAND_BUS_MASTER, 0);
                                }
                            }
                            #[cfg(z270_minimal_acpi)]
                            if bus == 1 && device == 0 && function <= 1 {
                                // GP102 and its HDMI function share the device.
                                // Assignment must not bypass the firmware DMA
                                // handoff. Preserve display scanout/MEM decode;
                                // do not issue a GPU or secondary-bus reset.
                                self.mask_pci_interrupts(config);
                                let ptr = (config + PCI_COMMAND_OFFSET) as *mut u16;
                                let old = unsafe { read_volatile(ptr) };
                                unsafe {
                                    write_volatile(ptr, (old & !PCI_COMMAND_BUS_MASTER) | PCI_COMMAND_INTX_DISABLE);
                                    asm!("mfence", options(nostack, preserves_flags));
                                }
                                let new = unsafe { read_volatile(ptr) };
                                assert_eq!(new & PCI_COMMAND_BUS_MASTER, 0,
                                    "assigned GPU still bus mastering before VT-d activation");
                                info!("assigned GPU quiesced {:02x}:{:02x}.{} cmd={:04x}->{:04x}",
                                    bus, device, function, old, new);
                            }
                            if base_class == 0x0c && sub_class == 0x03 && prog_if == 0x30 {
                                // Assigned does not mean quiescent: firmware may
                                // still have rings containing host addresses.
                                // Stop before switching the DMA address space.
                                self.stop_xhci(config, bus, device, function);
                                self.mask_pci_interrupts(config);
                                let ptr = (config + PCI_COMMAND_OFFSET) as *mut u16;
                                let old = unsafe { read_volatile(ptr) };
                                unsafe {
                                    write_volatile(ptr, (old & !PCI_COMMAND_BUS_MASTER) | PCI_COMMAND_INTX_DISABLE);
                                    asm!("mfence", options(nostack, preserves_flags));
                                }
                                let new = unsafe { read_volatile(ptr) };
                                assert_eq!(new & PCI_COMMAND_BUS_MASTER, 0,
                                    "assigned xHCI still bus mastering before VT-d activation");
                                info!("assigned xHCI quiesced {:02x}:{:02x}.{} cmd={:04x}->{:04x}",
                                    bus, device, function, old, new);
                            }
                            continue;
                        }

                        if base_class == 0x0c && sub_class == 0x03 && prog_if == 0x30 {
                            self.stop_xhci(config, bus, device, function);
                        }
                        self.mask_pci_interrupts(config);

                        let command_ptr = (config + PCI_COMMAND_OFFSET) as *mut u16;
                        let command = unsafe { read_volatile(command_ptr) };
                        let disabled_command =
                            (command & !PCI_COMMAND_BUS_MASTER) | PCI_COMMAND_INTX_DISABLE;
                        if command != disabled_command {
                            unsafe {
                                write_volatile(command_ptr, disabled_command);
                                asm!("mfence", options(nostack, preserves_flags));
                            };
                            let command_after = unsafe { read_volatile(command_ptr) };
                            warn!(
                                "PCI QUIESCE {:02x}:{:02x}.{} cmd {:04x}->{:04x}",
                                bus,
                                device,
                                function,
                                command,
                                command_after
                            );
                            if command_after & PCI_COMMAND_BUS_MASTER != 0 {
                                error!(
                                    "PCI BME STUCK {:02x}:{:02x}.{}",
                                    bus, device, function
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn mask_pci_interrupts(&self, config: usize) {
        let status = unsafe { read_volatile((config + PCI_STATUS_OFFSET) as *const u16) };
        if status & PCI_STATUS_CAPABILITIES_LIST == 0 {
            return;
        }

        let mut pointer = unsafe {
            read_volatile((config + PCI_CAPABILITIES_PTR_OFFSET) as *const u8) & !0x3
        };
        for _ in 0..48 {
            if !(0x40..=0xfc).contains(&pointer) {
                break;
            }
            let capability = config + pointer as usize;
            let capability_id = unsafe { read_volatile(capability as *const u8) };
            match capability_id {
                PCI_CAP_ID_MSI => {
                    let control = (capability + 2) as *mut u16;
                    let value = unsafe { read_volatile(control) };
                    unsafe { write_volatile(control, value & !1) };
                }
                PCI_CAP_ID_MSIX => {
                    let control = (capability + 2) as *mut u16;
                    let value = unsafe { read_volatile(control) };
                    unsafe { write_volatile(control, (value & !(1 << 15)) | (1 << 14)) };
                }
                _ => {}
            }
            pointer = unsafe { read_volatile((capability + 1) as *const u8) & !0x3 };
        }
    }

    fn stop_xhci(&self, config: usize, bus: u8, device: u8, function: u8) {
        let bar_low = unsafe { read_volatile((config + PCI_BAR0_OFFSET) as *const u32) };
        if bar_low & 1 != 0 {
            return;
        }
        let mut bar = (bar_low & !0xf) as u64;
        if (bar_low >> 1) & 0x3 == 0x2 {
            let bar_high = unsafe {
                read_volatile((config + PCI_BAR0_OFFSET + 4) as *const u32)
            };
            bar |= (bar_high as u64) << 32;
        }
        if bar == 0 {
            return;
        }

        let capability_length = unsafe { read_volatile(bar as *const u8) } as usize;
        let operational_base = bar as usize + capability_length;
        let command_ptr = operational_base as *mut u32;
        let status_ptr = (operational_base + 4) as *const u32;
        let command = unsafe { read_volatile(command_ptr) };
        if command & 1 != 0 {
            unsafe { write_volatile(command_ptr, command & !1) };
        }

        let start = current_time_nanos();
        while unsafe { read_volatile(status_ptr) } & 1 == 0 {
            if current_time_nanos().wrapping_sub(start) >= 100_000_000 {
                error!(
                    "xHCI HALT TIMEOUT {:02x}:{:02x}.{} usbcmd={:#x} usbsts={:#x}",
                    bus,
                    device,
                    function,
                    unsafe { read_volatile(command_ptr) },
                    unsafe { read_volatile(status_ptr) }
                );
                panic!("xHCI failed to halt before DMA address-space switch");
            }
            spin_loop();
        }
        info!("xHCI HALTED {:02x}:{:02x}.{}", bus, device, function);
    }

    fn wait_for_pci_dma_drain(&self) {
        let start = current_time_nanos();
        while current_time_nanos().wrapping_sub(start) < PCI_DMA_DRAIN_NS {
            spin_loop();
        }
        info!("unassigned PCI DMA drain interval complete");
    }

    fn report_faults(&self) -> bool {
        let fsts = self.mmio_read_u32(DMAR_FSTS_REG);
        if fsts == 0 {
            return false;
        }

        let cap = self.mmio_read_u64(DMAR_CAP_REG);
        let fault_record_offset = (((cap >> 24) & 0x3ff) as usize) * 16;
        let fault_record_count = (((cap >> 40) & 0xff) as usize) + 1;
        error!(
            "VT-d DMA fault: base={:#x}, fsts={:#010x}, records={}, fro={:#x}",
            self.reg_base_hpa, fsts, fault_record_count, fault_record_offset
        );
        for index in 0..fault_record_count {
            let offset = fault_record_offset + index * 16;
            let lo = self.mmio_read_u64(offset);
            let hi = self.mmio_read_u64(offset + 8);
            if hi.get_bit(63) {
                error!(
                    "VT-d FRCD[{}]: sid={:02x}:{:02x}.{}, reason={:#04x}, addr={:#x}, hi={:#018x}, lo={:#018x}",
                    index,
                    ((hi & 0xffff) >> 8) as u8,
                    ((hi & 0xff) >> 3) as u8,
                    (hi & 0x7) as u8,
                    ((hi >> 32) & 0xff) as u8,
                    lo & !0xfff,
                    hi,
                    lo
                );
                // Release this record so a later requester (especially AHCI)
                // can be captured rather than remaining hidden behind xHCI.
                self.mmio_write_u64(offset + 8, 1 << 63);
            }
        }
        self.mmio_write_u32(DMAR_FSTS_REG, fsts);
        true
    }

    fn wait(&mut self, mask: GstsFlags, cond: bool) {
        const VTD_WAIT_TIMEOUT_NS: u64 = 1_000_000_000;
        let start = current_time_nanos();
        loop {
            spin_loop();
            let status = self.mmio_read_u32(DMAR_GSTS_REG);
            if GstsFlags::from_bits_truncate(status).contains(mask) != cond {
                break;
            }
            if current_time_nanos().wrapping_sub(start) > VTD_WAIT_TIMEOUT_NS {
                panic!(
                    "VT-d status timeout: base={:#x}, gsts={:#x}, mask={:#x}, target_set={}",
                    self.reg_base_hpa,
                    status,
                    mask.bits(),
                    !cond
                );
            }
        }
    }

    fn mmio_read_u32(&self, reg: usize) -> u32 {
        unsafe { read_volatile((self.reg_base_hpa + reg) as *const u32) }
    }

    fn mmio_read_u64(&self, reg: usize) -> u64 {
        unsafe { read_volatile((self.reg_base_hpa + reg) as *const u64) }
    }

    fn mmio_write_u32(&self, reg: usize, value: u32) {
        unsafe { write_volatile((self.reg_base_hpa + reg) as *mut u32, value) };
    }

    fn mmio_write_u64(&self, reg: usize, value: u64) {
        unsafe { write_volatile((self.reg_base_hpa + reg) as *mut u64, value) };
    }
}

const fn dma_ccmd_sid(sid: u16) -> u64 {
    ((sid as u64) & 0xffff) << 32
}

const fn dma_ccmd_did(did: u16) -> u64 {
    ((did as u64) & 0xffff) << 16
}

const fn dma_ccmd_fm(fm: u8) -> u64 {
    ((fm as u64) & 0x3) << 48
}

const fn dma_iotlb_did(did: u16) -> u64 {
    ((did as u64) & 0xffff) << 16
}

fn parse_root_dmar() -> Mutex<Vtd> {
    let dmar = acpi::root_get_table(&Signature::DMAR).unwrap();
    let mut cur: usize = 48; // start offset of remapping structures
    let len = dmar.get_len();

    let mut reg_base_hpa: usize = 0;

    while cur < len {
        let struct_type = dmar.get_u16(cur);
        let struct_len = dmar.get_u16(cur + 2) as usize;

        if struct_type == 0 {
            let segment = dmar.get_u16(cur + 6);

            // we only support segment 0
            if segment == 0 {
                reg_base_hpa = dmar.get_u64(cur + 8) as usize;
            }
        }
        cur += struct_len;
    }

    assert!(reg_base_hpa != 0);

    Mutex::new(Vtd {
        reg_base_hpa,
        devices: BTreeMap::new(),
        root_table: Frame::new_zero().unwrap(),
        context_tables: BTreeMap::new(),
        qi_queue: Frame::new_zero().unwrap(),
        qi_completion: Frame::new_zero().unwrap(),
        ir_table: Frame::new().unwrap(),
        gcmd: GcmdFlags::empty(),
        qi_queue_hpa: 0,
        qi_tail: 0,
    })
}

// called after acpi init
pub fn iommu_init() {
    VTD.call_once(|| parse_root_dmar());
    VTD.get().unwrap().lock().init();
    // init_msi_cap_hpa_space();
}

pub fn iommu_add_device(zone_id: usize, bdf: usize, _: usize) {
    // info!("vtd add device: {:x}, zone: {:x}", bdf, zone_id);
    VTD.get().unwrap().lock().add_device(zone_id, bdf as _);
}

pub fn clear_dma_translation_tables(zone_id: usize) {
    VTD.get().unwrap().lock().clear_devices(zone_id);
}

pub fn fill_dma_translation_tables(zone_id: usize, zone_s2pt_hpa: HostPhysAddr) {
    VTD.get()
        .unwrap()
        .lock()
        .fill_dma_translation_tables(zone_id, zone_s2pt_hpa);
}

/// should be called after gpm is activated
pub fn activate() {
    VTD.get().unwrap().lock().activate();
}

pub fn flush(zone_id: usize, bus: u8, dev_func: u8) {
    VTD.get().unwrap().lock().flush(zone_id, bus, dev_func);
}

/// Poll VT-d fault state at a low rate.  Fault interrupts remain masked until
/// a real fault-event vector is installed, so this is the bring-up diagnostic
/// path used from VM-exit.
pub fn check_faults() {
    if FAULT_REPORT_COUNT.load(Ordering::Relaxed) >= MAX_FAULT_REPORTS {
        return;
    }

    let now = current_time_nanos();
    let last = LAST_FAULT_CHECK_NS.load(Ordering::Relaxed);
    if now.wrapping_sub(last) < 100_000_000
        || LAST_FAULT_CHECK_NS
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return;
    }

    if VTD.get().unwrap().lock().report_faults() {
        FAULT_REPORT_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline]
fn pci_ecam_function(ecam_base: usize, bus: u8, device: u8, function: u8) -> usize {
    ecam_base + ((bus as usize) << 20) + ((device as usize) << 15) + ((function as usize) << 12)
}

fn flush_cache_range(hpa: usize, size: usize) {
    if size == 0 {
        return;
    }
    let end = hpa.checked_add(size).expect("cache flush range overflow");
    let mut addr = hpa & !63usize;
    while addr < end {
        // CLFLUSH is available on the supported VMX platforms, unlike
        // CLFLUSHOPT on older CPUs. Include partially covered cache lines.
        unsafe { asm!("clflush [{addr}]", addr = in(reg) addr) };
        addr = addr.checked_add(64).expect("cache flush alignment overflow");
    }
    // Order writeback/invalidation before both publication and status loads.
    unsafe { asm!("mfence", options(nostack, preserves_flags)) };
}
