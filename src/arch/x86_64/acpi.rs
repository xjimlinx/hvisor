// Copyright (c) 2025 Syswonder
// Z270 ACPI routing revisions are intentionally tracked in the board DSDT.
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
    arch::boot,
    config::{HvConfigMemoryRegion, HvZoneConfig},
    cpu_data::{this_zone, CpuSet},
    error::HvResult,
    memory::addr::{phys_to_virt, virt_to_phys},
    platform::ROOT_PCI_MAX_BUS,
};
use acpi::{
    fadt::Fadt,
    madt::{LocalApicEntry, Madt, MadtEntry},
    mcfg::{Mcfg, McfgEntry},
    rsdp::Rsdp,
    sdt::{SdtHeader, Signature},
    AcpiHandler, AcpiTables, PciConfigRegions,
};
use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};
use core::{
    any::Any,
    mem::size_of,
    pin::Pin,
    ptr::{read_unaligned, write_unaligned, NonNull},
};
use spin::{Mutex, Once};

#[path = "acpi_image.rs"]
mod acpi_image;

const RSDP_V1_SIZE: usize = 20;
const RSDP_V2_SIZE: usize = 36;

const RSDP_RSDT_OFFSET: usize = 16;
const RSDP_RSDT_PTR_SIZE: usize = 4;
const RSDT_PTR_SIZE: usize = 4;

const FADT_DSDT_OFFSET_32: usize = 0x28;
const FADT_DSDT_OFFSET_64: usize = 0x8c;

const FADT_FACS_OFFSET_32: usize = 0x24;
const FADT_FACS_OFFSET_64: usize = 0x84;

const SDT_HEADER_SIZE: usize = 36;

const RSDP_CHECKSUM_OFFSET: usize = 8;
const RSDP_REVISION_OFFSET: usize = 15;
const ACPI_CHECKSUM_OFFSET: usize = 9;
const X86_DIRECT_MAP_SIZE: usize = 1usize << 39;
const MAX_ACPI_TABLE_SIZE: usize = 16 * 1024 * 1024;

macro_rules! acpi_table {
    ($a: ident, $b: ident) => {
        #[repr(transparent)]
        struct $a {
            header: SdtHeader,
        }

        unsafe impl acpi::AcpiTable for $a {
            const SIGNATURE: Signature = Signature::$b;
            fn header(&self) -> &SdtHeader {
                &self.header
            }
        }
    };
}

#[derive(Clone, Debug)]
struct HvAcpiHandler {}

impl AcpiHandler for HvAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let virtual_address = phys_to_virt(physical_address);
        acpi::PhysicalMapping::new(
            physical_address,
            NonNull::new(virtual_address as *mut T).unwrap(),
            size,
            size,
            self.clone(),
        )
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {}
}

static ROOT_ACPI: Once<RootAcpi> = Once::new();

#[derive(Clone, Debug)]
enum PatchValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
}

#[derive(Clone, Debug, Default)]
pub struct AcpiTable {
    sig: Option<Signature>,
    src: usize,
    patches: BTreeMap<usize, PatchValue>,
    len: usize,
    checksum_offset: Option<usize>,
    gpa: usize,
    hpa: usize,
    is_addr_set: bool,
}

impl AcpiTable {
    pub fn set_u8(&mut self, value: u8, offset: usize) {
        assert!(offset < self.len, "ACPI byte patch out of bounds");
        self.patches.insert(offset, PatchValue::U8(value));
    }

    pub fn set_u32(&mut self, value: u32, offset: usize) {
        assert!(offset.checked_add(4).is_some_and(|end| end <= self.len), "ACPI dword patch out of bounds");
        self.patches.insert(offset, PatchValue::U32(value));
    }

    pub fn set_u64(&mut self, value: u64, offset: usize) {
        assert!(offset.checked_add(8).is_some_and(|end| end <= self.len), "ACPI qword patch out of bounds");
        self.patches.insert(offset, PatchValue::U64(value));
    }

    /// new len must not be longer
    pub fn set_new_len(&mut self, len: usize) {
        let src_len = self.get_u32(4) as usize;
        println!("len: {:x}, selflen: {:x}", len, src_len);
        assert!(len <= src_len);

        assert!(len >= SDT_HEADER_SIZE);
        self.set_u32(len as _, 4);
        self.len = len;
    }

    pub fn get_len(&self) -> usize {
        self.len
    }

    pub fn get_unpatched_src(&self) -> *const u8 {
        self.src as *const u8
    }

    pub fn get_u8(&self, offset: usize) -> u8 {
        if let Some(&PatchValue::U8(value)) = self.patches.get(&offset) {
            return value;
        }
        unsafe { *((self.src + offset) as *const u8) }
    }

    pub fn get_u16(&self, offset: usize) -> u16 {
        if let Some(&PatchValue::U16(value)) = self.patches.get(&offset) {
            return value;
        }
        unsafe { read_unaligned((self.src + offset) as *const u16) }
    }

    pub fn get_u32(&self, offset: usize) -> u32 {
        if let Some(&PatchValue::U32(value)) = self.patches.get(&offset) {
            return value;
        }
        unsafe { read_unaligned((self.src + offset) as *const u32) }
    }

    pub fn get_u64(&self, offset: usize) -> u64 {
        if let Some(&PatchValue::U64(value)) = self.patches.get(&offset) {
            return value;
        }
        unsafe { read_unaligned((self.src + offset) as *const u64) }
    }

    pub fn fill(
        &mut self,
        sig: Option<Signature>,
        ptr: *const u8,
        len: usize,
        checksum_offset: usize,
    ) {
        self.sig = sig;
        self.patches.clear();
        self.src = ptr as usize;
        self.len = len;
        // FACS has no SDT checksum: bytes 8..12 are its hardware signature.
        self.checksum_offset = if sig == Some(Signature::FACS) {
            None
        } else {
            assert!(checksum_offset < len);
            Some(checksum_offset)
        };
    }

    pub unsafe fn copy_to_mem(&self) {
        core::ptr::copy(self.src as *const u8, self.hpa as *mut u8, self.len);

        macro_rules! write_patch {
            ($addr:expr, $val:expr, $ty:ty) => {
                write_unaligned($addr as *mut $ty, $val)
            };
        }

        for (offset, value) in self.patches.iter() {
            let width = match value {
                PatchValue::U8(_) => 1,
                PatchValue::U16(_) => 2,
                PatchValue::U32(_) => 4,
                PatchValue::U64(_) => 8,
            };
            assert!(offset.checked_add(width).is_some_and(|end| end <= self.len),
                "ACPI patch outside {:?}: offset={:#x}, width={}, len={:#x}",
                self.sig, offset, width, self.len);
            let addr = self.hpa + *offset;
            match *value {
                PatchValue::U8(v) => write_patch!(addr, v, u8),
                PatchValue::U16(v) => write_patch!(addr, v, u16),
                PatchValue::U32(v) => write_patch!(addr, v, u32),
                PatchValue::U64(v) => write_patch!(addr, v, u64),
                _ => {}
            }
        }
        // Recompute from the final image, after all replacements/truncation.
        // Never mutate the source firmware tables shared by other zones.
        acpi_image::finish_checksum(
            core::slice::from_raw_parts_mut(self.hpa as *mut u8, self.len),
            self.checksum_offset,
        );
    }

    pub fn set_addr(&mut self, hpa: usize, gpa: usize) {
        self.hpa = hpa;
        self.gpa = gpa;
        self.is_addr_set = true;
    }

}

#[derive(Copy, Clone, Debug)]
struct AcpiPointer {
    pub from_sig: Signature,
    pub from_offset: usize,
    pub to_sig: Signature,
    pub pointer_size: usize,
}

#[derive(Clone, Debug, Default)]
pub struct RootAcpi {
    /// we need to store rsdp to a safer place
    rsdp_copy: Vec<u8>,
    rsdp: AcpiTable,
    tables: BTreeMap<Signature, AcpiTable>,
    ssdts: BTreeMap<usize, AcpiTable>,
    pointers: Vec<AcpiPointer>,
    config_space_base: usize,
    config_space_size: usize,
    /// key: apic id, value: cpu id (continuous)
    apic_id_to_cpu_id: BTreeMap<usize, usize>,
    /// key: cpu id (continuous), value: apic id
    cpu_id_to_apic_id: BTreeMap<usize, usize>,
}

impl RootAcpi {
    fn add_pointer(
        &mut self,
        from_sig: Signature,
        from_offset: usize,
        to_sig: Signature,
        pointer_size: usize,
    ) {
        self.pointers.push(AcpiPointer {
            from_sig,
            from_offset,
            to_sig,
            pointer_size,
        });
    }

    fn add_new_table(&mut self, sig: Signature, ptr: *const u8, len: usize) {
        let mut table = AcpiTable::default();
        table.fill(Some(sig), ptr, len, ACPI_CHECKSUM_OFFSET);
        self.tables.insert(sig, table);
    }

    fn add_ssdt(&mut self, ptr: *const u8, len: usize, rsdt_offset: usize) {
        let mut table = AcpiTable::default();
        table.fill(Some(Signature::SSDT), ptr, len, ACPI_CHECKSUM_OFFSET);
        self.ssdts.insert(rsdt_offset, table);
    }

    fn get_mut_table(&mut self, sig: Signature) -> Option<&mut AcpiTable> {
        self.tables.get_mut(&sig)
    }

    fn get_table(&self, sig: &Signature) -> Option<AcpiTable> {
        if self.tables.contains_key(sig) {
            Some(self.tables.get(sig).unwrap().clone())
        } else {
            None
        }
    }

    pub fn copy_to_zone_region(
        &self,
        rsdp_zone_region: &HvConfigMemoryRegion,
        acpi_zone_region: &HvConfigMemoryRegion,
        banned_tables: &BTreeSet<Signature>,
        cpu_set: &CpuSet,
        minimal: bool,
    ) {
        let mut rsdp = self.rsdp.clone();
        let mut tables = self.tables.clone();
        let mut ssdts = self.ssdts.clone();
        let mut pointers = self.pointers.clone();
        // Owned image remains alive until every guest table has been copied.
        let mut minimal_rsdt: Vec<u8> = Vec::new();
        #[cfg(z270_minimal_acpi)]
        if minimal {
            let dsdt = include_bytes!(concat!(env!("OUT_DIR"), "/z270-minimal-dsdt.aml"));
            let mut table = AcpiTable::default();
            table.fill(Some(Signature::DSDT), dsdt.as_ptr(), dsdt.len(), ACPI_CHECKSUM_OFFSET);
            tables.insert(Signature::DSDT, table);
            ssdts.clear();
            // Compact the RSDT: do not leave null pointers to removed SSDTs.
            let mut offset = SDT_HEADER_SIZE;
            for pointer in pointers.iter_mut() {
                if pointer.from_sig == Signature::RSDT && pointer.to_sig != Signature::RSDT {
                    pointer.from_offset = offset;
                    offset += RSDT_PTR_SIZE;
                }
            }
            let original = tables.get(&Signature::RSDT).unwrap();
            minimal_rsdt.extend_from_slice(unsafe {
                core::slice::from_raw_parts(original.get_unpatched_src(), SDT_HEADER_SIZE)
            });
            minimal_rsdt.resize(offset, 0);
            minimal_rsdt[4..8].copy_from_slice(&(offset as u32).to_le_bytes());
            let mut table = AcpiTable::default();
            table.fill(Some(Signature::RSDT), minimal_rsdt.as_ptr(), offset, ACPI_CHECKSUM_OFFSET);
            tables.insert(Signature::RSDT, table);
            assert!(pointers.iter().any(|p| p.to_sig == Signature::DSDT));
            info!("Z270 minimal ACPI: generated DSDT, no firmware SSDTs/OperationRegions");
        }

        // set rsdp addr
        rsdp.set_addr(
            rsdp_zone_region.physical_start as _,
            rsdp_zone_region.virtual_start as _,
        );

        let mut madt_cur: usize = SDT_HEADER_SIZE + 8;
        let mut madt = tables.get_mut(&Signature::MADT).unwrap();

        // fix madt cpu info
        for entry in
            unsafe { Pin::new_unchecked(&*(madt.get_unpatched_src() as *const Madt)) }.entries()
        {
            let mut entry_len = madt.get_u8(madt_cur + 1) as usize;
            match entry {
                MadtEntry::LocalApic(entry) => {
                    let mut disable_lapic = true;
                    if contains_apic_id(entry.apic_id as _) {
                        let cpuid = get_cpu_id(entry.apic_id as _);
                        if cpu_set.contains_cpu(cpuid) {
                            disable_lapic = false;
                        }
                        // reset processor id
                        madt.set_u8(cpuid as _, madt_cur + 2);
                    }
                    if disable_lapic {
                        // set flag to disable lapic
                        madt.set_u32(0x0, madt_cur + 4);
                    }
                }
                MadtEntry::LocalX2Apic(entry) => {
                    if !cpu_set.contains_cpu(entry.processor_uid as _) {}
                }
                _ => {}
            }
            madt_cur += entry_len;
        }

        // set pointers
        let hpa_start = acpi_zone_region.physical_start as usize;
        let gpa_start = acpi_zone_region.virtual_start as usize;
        let mut cur: usize = 0;
        assert!(rsdp.get_len() <= rsdp_zone_region.size as usize);
        assert_eq!((hpa_start | gpa_start) & 63, 0, "ACPI region must be 64-byte aligned");

        let mut tables_involved = BTreeSet::<Signature>::new();

        for pointer in pointers.iter() {
            let to = tables.get_mut(&pointer.to_sig).unwrap();
            tables_involved.insert(pointer.to_sig);

            if !to.is_addr_set {
                cur = acpi_image::reserve_table(
                    cur, to.get_len(), acpi_zone_region.size as usize,
                    if pointer.to_sig == Signature::FACS { 64 } else { 8 },
                ).expect("ACPI tables exceed guest region");
                info!(
                    "sig: {:x?}, hpa: {:x?}, gpa: {:x?}, size: {:x?}",
                    pointer.to_sig,
                    hpa_start + cur,
                    gpa_start + cur,
                    to.get_len()
                );
                to.set_addr(hpa_start + cur, gpa_start + cur);
                cur += to.get_len();
            }

            let to_gpa = match banned_tables.contains(&pointer.to_sig) {
                true => 0,
                false => to.gpa,
            };

            let from = match pointer.from_sig == pointer.to_sig {
                true => &mut rsdp,
                false => tables.get_mut(&pointer.from_sig).unwrap(),
            };

            match pointer.pointer_size {
                4 => {
                    from.set_u32(to_gpa as _, pointer.from_offset);
                }
                8 => {
                    from.set_u64(to_gpa as _, pointer.from_offset);
                }
                _ => {
                    warn!("Unused pointer size!");
                }
            }
        }

        let ban_ssdt = banned_tables.contains(&Signature::SSDT);
        let from = tables.get_mut(&Signature::RSDT).unwrap();
        for (&offset, ssdt) in ssdts.iter_mut() {
            cur = acpi_image::reserve_table(cur, ssdt.get_len(), acpi_zone_region.size as usize, 8)
                .expect("SSDTs exceed guest ACPI region");
            info!(
                "sig: {:x?}, hpa: {:x?}, gpa: {:x?}, size: {:x?}",
                Signature::SSDT,
                hpa_start + cur,
                gpa_start + cur,
                ssdt.get_len()
            );
            ssdt.set_addr(hpa_start + cur, gpa_start + cur);
            cur += ssdt.get_len();

            let to_gpa = match ban_ssdt {
                true => 0,
                false => ssdt.gpa,
            };
            from.set_u32(to_gpa as _, offset);
        }

        // copy to memory
        unsafe { rsdp.copy_to_mem() };
        for (sig, table) in tables.iter() {
            // don't copy tables that are not inside ACPI tree
            if tables_involved.contains(sig) {
                unsafe { table.copy_to_mem() };
            }
        }
        if !ban_ssdt {
            for (&offset, ssdt) in ssdts.iter() {
                unsafe { ssdt.copy_to_mem() };
            }
        }
    }

    // let zone 0 bsp cpu does the work
    pub fn init() -> Self {
        let mut root_acpi = Self::default();
        let rsdp_addr = boot::get_multiboot_tags().rsdp_addr.unwrap();

        // Multiboot2 tag 14 contains exactly the 20-byte ACPI 1.0 portion of
        // the firmware RSDP.  Do not read a full ACPI 2.0 Rsdp from it: the
        // bytes after the tag belong to the next Multiboot tag.  This x86
        // implementation builds an RSDT with 32-bit pointers, so expose a
        // checksum-correct ACPI 1.0 view even when UEFI left revision=2 in
        // those first 20 bytes.
        root_acpi.rsdp_copy = vec![0u8; core::mem::size_of::<Rsdp>()];
        unsafe {
            core::ptr::copy_nonoverlapping(
                rsdp_addr as *const u8,
                root_acpi.rsdp_copy.as_mut_ptr(),
                RSDP_V1_SIZE,
            );
        }
        let firmware_revision = root_acpi.rsdp_copy[RSDP_REVISION_OFFSET];
        let rsdt_addr = u32::from_le_bytes(
            root_acpi.rsdp_copy[RSDP_RSDT_OFFSET..RSDP_RSDT_OFFSET + RSDP_RSDT_PTR_SIZE]
                .try_into()
                .unwrap(),
        );
        println!(
            "ACPI v1 tag: firmware revision={}, RSDT={:#x}",
            firmware_revision, rsdt_addr
        );
        assert!(rsdt_addr != 0);
        root_acpi.rsdp_copy[RSDP_REVISION_OFFSET] = 0;
        root_acpi.rsdp_copy[RSDP_CHECKSUM_OFFSET] = 0;
        let checksum = root_acpi.rsdp_copy[..RSDP_V1_SIZE]
            .iter()
            .fold(0u8, |sum, &byte| sum.wrapping_add(byte));
        root_acpi.rsdp_copy[RSDP_CHECKSUM_OFFSET] = 0u8.wrapping_sub(checksum);
        let rsdp_copy_vaddr = root_acpi.rsdp_copy.as_ptr() as usize;
        let rsdp_copy_paddr = virt_to_phys(rsdp_copy_vaddr);

        let handler = HvAcpiHandler {};
        let rsdp_mapping = unsafe {
            handler.map_physical_region::<Rsdp>(rsdp_copy_paddr, core::mem::size_of::<Rsdp>())
        };

        // The tag was deliberately normalized to the ACPI 1.0/RSDT view.
        assert!(rsdp_mapping.revision() == 0);

        root_acpi.rsdp.fill(
            None,
            rsdp_mapping.virtual_start().as_ptr() as *const u8,
            RSDP_V1_SIZE,
            RSDP_CHECKSUM_OFFSET,
        );
        root_acpi.add_pointer(
            Signature::RSDT,
            RSDP_RSDT_OFFSET,
            Signature::RSDT,
            RSDP_RSDT_PTR_SIZE,
        );

        // get rsdt
        let rsdt_addr = rsdp_mapping.rsdt_address() as usize;
        root_acpi.add_new_table(Signature::RSDT, rsdt_addr as *const u8, SDT_HEADER_SIZE);
        let mut rsdt_offset = root_acpi.get_mut_table(Signature::RSDT).unwrap().get_len();

        let tables =
            unsafe { AcpiTables::from_validated_rsdp(HvAcpiHandler {}, rsdp_mapping) }.unwrap();

        // print rsdt entries
        let mut rsdt_entry = rsdt_addr + 36;
        let size = (unsafe { *((rsdt_addr + 4) as *const u32) } as usize - 36) / 4;
        for i in 0..size {
            let addr = unsafe { *(rsdt_entry as *const u32) } as usize;
            let sig_ptr = addr as *const u8;
            let sig =
                unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(sig_ptr, 4)) };

            println!("sig: {:#x?} ptr: {:x} len: {:x}", sig, addr, unsafe {
                *((addr + 4) as *const u32)
            });
            rsdt_entry += 4;
        }

        // mcfg
        if let Ok(mcfg) = tables.find_table::<Mcfg>() {
            root_acpi.add_new_table(
                Signature::MCFG,
                mcfg.physical_start() as *const u8,
                mcfg.region_length(),
            );

            println!("---------- MCFG ----------");
            let mut offset = size_of::<Mcfg>() + 0xb;

            if let Some(entry) = mcfg
                .entries()
                .iter()
                .find(|&entry| entry.pci_segment_group == 0)
            {
                // we only support segment group 0
                println!("{:x?}", entry);

                let max_bus = ROOT_PCI_MAX_BUS as u8;
                // update bus_number_end
                root_acpi
                    .get_mut_table(Signature::MCFG)
                    .unwrap()
                    .set_u8(max_bus, offset);
                offset += size_of::<McfgEntry>();

                root_acpi.config_space_base = entry.base_address as _;
                root_acpi.config_space_size =
                    (((max_bus as u64 - entry.bus_number_start as u64) + 1) << 20) as usize;
            }

            root_acpi.add_pointer(Signature::RSDT, rsdt_offset, Signature::MCFG, RSDT_PTR_SIZE);
            rsdt_offset += RSDT_PTR_SIZE;
        }

        // fadt
        if let Ok(fadt) = tables.find_table::<Fadt>() {
            root_acpi.add_new_table(
                Signature::FADT,
                fadt.physical_start() as *const u8,
                fadt.region_length(),
            );

            println!("---------- FADT ----------");

            root_acpi.add_pointer(Signature::RSDT, rsdt_offset, Signature::FADT, RSDT_PTR_SIZE);
            rsdt_offset += RSDT_PTR_SIZE;

            // acpi
            let sci_int = fadt.sci_interrupt;
            let smi_port = fadt.smi_cmd_port;
            let acpi_enable = fadt.acpi_enable;
            let acpi_disable = fadt.acpi_disable;
            let pm1a_con = fadt.pm1a_control_block();
            let pm1a_evt = fadt.pm1a_event_block();

            /*println!(
                "sci_interrupt: {:x}, smi_cmd_port: {:x}, acpi_enable: {:x}, acpi_disable: {:x}, pm1a_con: {:#x?}, pm1a_evt: {:#x?}",
                sci_int, smi_port, acpi_enable, acpi_disable, pm1a_con, pm1a_evt,
            );*/
            // println!("{:#x?}", fadt.get());
            // loop {}

            // dsdt
            // Read the FADT address fields by their ACPI-defined byte offsets.
            // This avoids relying on a host Rust representation for a packed
            // firmware table. Prefer the 64-bit field, with the compatibility
            // 32-bit field as fallback.
            let fadt_vaddr = fadt.virtual_start().as_ptr().cast::<u8>();
            let fadt_len = fadt.region_length();
            let read_fadt_address = |offset32: usize, offset64: usize| -> Option<usize> {
                let addr32 = if fadt_len >= offset32 + size_of::<u32>() {
                    unsafe { read_unaligned(fadt_vaddr.add(offset32).cast::<u32>()) as usize }
                } else {
                    0
                };
                let addr64 = if fadt_len >= offset64 + size_of::<u64>() {
                    unsafe { read_unaligned(fadt_vaddr.add(offset64).cast::<u64>()) }
                } else {
                    0
                };
                println!(
                    "FADT address fields [{:#x}/{:#x}]: 32={:#x}, 64={:#x}",
                    offset32, offset64, addr32, addr64
                );
                [usize::try_from(addr64).ok(), Some(addr32)]
                    .into_iter()
                    .flatten()
                    .find(|&addr| addr != 0 && addr < X86_DIRECT_MAP_SIZE)
            };

            if let Some(dsdt_addr) =
                read_fadt_address(FADT_DSDT_OFFSET_32, FADT_DSDT_OFFSET_64)
            {
                let dsdt_ptr = phys_to_virt(dsdt_addr) as *const u8;
                let signature = unsafe { read_unaligned(dsdt_ptr.cast::<u32>()) };
                let dsdt_len = unsafe { read_unaligned(dsdt_ptr.add(4).cast::<u32>()) as usize };
                let end_is_mapped = dsdt_addr
                    .checked_add(dsdt_len)
                    .is_some_and(|end| end <= X86_DIRECT_MAP_SIZE);
                let valid_header = signature == u32::from_le_bytes(*b"DSDT")
                    && (SDT_HEADER_SIZE..=MAX_ACPI_TABLE_SIZE).contains(&dsdt_len)
                    && end_is_mapped;
                let valid_checksum = valid_header
                    && unsafe { core::slice::from_raw_parts(dsdt_ptr, dsdt_len) }
                        .iter()
                        .fold(0u8, |sum, &byte| sum.wrapping_add(byte))
                        == 0;

                println!(
                    "DSDT candidate: ptr={:#x}, len={:#x}, header={}, checksum={}",
                    dsdt_addr, dsdt_len, valid_header, valid_checksum
                );
                if valid_checksum {
                    root_acpi.add_new_table(Signature::DSDT, dsdt_ptr, dsdt_len);
                    root_acpi.add_pointer(
                        Signature::FADT,
                        FADT_DSDT_OFFSET_32,
                        Signature::DSDT,
                        4,
                    );
                    // The legacy RSDT may reference a 132-byte FADT even
                    // when the firmware's XSDT has a longer FADT. Do not write
                    // absent extended fields into the following guest table.
                    if fadt_len >= FADT_DSDT_OFFSET_64 + 8 {
                        root_acpi.add_pointer(
                            Signature::FADT,
                            FADT_DSDT_OFFSET_64,
                            Signature::DSDT,
                            8,
                        );
                    }
                }
            }

            // facs
            if let Some(facs_addr) =
                read_fadt_address(FADT_FACS_OFFSET_32, FADT_FACS_OFFSET_64)
            {
                let facs_ptr = phys_to_virt(facs_addr) as *const u8;
                let signature = unsafe { read_unaligned(facs_ptr.cast::<u32>()) };
                let facs_len = unsafe { read_unaligned(facs_ptr.add(4).cast::<u32>()) as usize };
                let valid = signature == u32::from_le_bytes(*b"FACS")
                    && (8..=MAX_ACPI_TABLE_SIZE).contains(&facs_len)
                    && facs_addr
                        .checked_add(facs_len)
                        .is_some_and(|end| end <= X86_DIRECT_MAP_SIZE);
                println!(
                    "FACS candidate: ptr={:#x}, len={:#x}, valid={}",
                    facs_addr, facs_len, valid
                );
                if valid {
                    root_acpi.add_new_table(Signature::FACS, facs_ptr, facs_len);
                    root_acpi.add_pointer(
                        Signature::FADT,
                        FADT_FACS_OFFSET_32,
                        Signature::FACS,
                        4,
                    );
                    if fadt_len >= FADT_FACS_OFFSET_64 + 8 {
                        root_acpi.add_pointer(
                            Signature::FADT,
                            FADT_FACS_OFFSET_64,
                            Signature::FACS,
                            8,
                        );
                    }
                }
            }
        }

        // madt
        if let Ok(madt) = tables.find_table::<Madt>() {
            root_acpi.add_new_table(
                Signature::MADT,
                madt.physical_start() as *const u8,
                madt.region_length(),
            );

            println!("---------- MADT ----------");
            for entry in madt.get().entries() {
                match entry {
                    MadtEntry::LocalApic(entry) => {
                        if entry.flags != 0 {
                            println!("{:x?}", entry);
                            let cpu_id = root_acpi.apic_id_to_cpu_id.len();
                            root_acpi
                                .apic_id_to_cpu_id
                                .insert(entry.apic_id as _, cpu_id);
                            root_acpi
                                .cpu_id_to_apic_id
                                .insert(cpu_id, entry.apic_id as _);
                        }
                    }
                    _ => {}
                }
            }

            root_acpi.add_pointer(Signature::RSDT, rsdt_offset, Signature::MADT, RSDT_PTR_SIZE);
            rsdt_offset += RSDT_PTR_SIZE;
        }

        // dmar
        // Retain the physical timer description; this is not firmware AML.
        #[cfg(z270_minimal_acpi)]
        {
            acpi_table!(HpetTable, HPET);
            if let Ok(hpet) = tables.find_table::<HpetTable>() {
                root_acpi.add_new_table(Signature::HPET, hpet.physical_start() as *const u8, hpet.region_length());
                root_acpi.add_pointer(Signature::RSDT, rsdt_offset, Signature::HPET, RSDT_PTR_SIZE);
                rsdt_offset += RSDT_PTR_SIZE;
            }
        }
        acpi_table!(Dmar, DMAR);
        if let Ok(dmar) = tables.find_table::<Dmar>() {
            root_acpi.add_new_table(
                Signature::DMAR,
                dmar.physical_start() as *const u8,
                dmar.region_length(),
            );

            /*println!("DMAR: {:x?}", unsafe {
                *((dmar.physical_start() + 56) as *const [u8; 8])
            });*/

            // self.add_pointer(Signature::RSDT, rsdt_offset, Signature::DMAR, RSDT_PTR_SIZE);
            // rsdt_offset += RSDT_PTR_SIZE;
        }

        // ssdt
        for ssdt in tables.ssdts() {
            root_acpi.add_ssdt(
                (ssdt.address - SDT_HEADER_SIZE) as *const u8,
                (ssdt.length as usize + SDT_HEADER_SIZE),
                rsdt_offset,
            );
            rsdt_offset += RSDT_PTR_SIZE;
        }

        if let Some(rsdt) = root_acpi.get_mut_table(Signature::RSDT) {
            rsdt.set_new_len(rsdt_offset);
        }
        root_acpi
    }
}

// let zone 0 bsp cpu does the work
pub fn root_init() {
    ROOT_ACPI.call_once(|| RootAcpi::init());
}

pub fn copy_to_guest_memory_region(config: &HvZoneConfig, cpu_set: &CpuSet) {
    let mut banned: BTreeSet<Signature> = BTreeSet::new();
    if config.zone_id != 0 {
        // Guest zones must not evaluate arbitrary firmware SSDTs: their
        // OperationRegions target host-owned controllers.  The Z270 guest
        // still needs the FADT header so Linux can discover the synthetic
        // PCI0._PRT route used by a passthrough display device.
        banned.insert(Signature::SSDT);
        if !cfg!(z270_minimal_acpi) {
            banned.insert(Signature::FADT);
        }
    }
    ROOT_ACPI.get().unwrap().copy_to_zone_region(
        &config.memory_regions()[config.arch_config.rsdp_memory_region_id],
        &config.memory_regions()[config.arch_config.acpi_memory_region_id],
        &banned,
        cpu_set,
        cfg!(z270_minimal_acpi) && (config.zone_id == 0 || config.zone_id == 1),
    );
}

pub fn root_get_table(sig: &Signature) -> Option<AcpiTable> {
    ROOT_ACPI.get().unwrap().get_table(sig)
}

pub fn root_get_config_space_info() -> Option<(usize, usize)> {
    let acpi = ROOT_ACPI.get().unwrap();
    Some((acpi.config_space_base, acpi.config_space_size))
}

pub fn try_get_cpu_id(apic_id: usize) -> Option<usize> {
    ROOT_ACPI
        .get()
        .and_then(|acpi| acpi.apic_id_to_cpu_id.get(&apic_id))
        .copied()
}

fn contains_apic_id(apic_id: usize) -> bool {
    ROOT_ACPI
        .get()
        .unwrap()
        .apic_id_to_cpu_id
        .contains_key(&apic_id)
}

pub fn get_cpu_id(apic_id: usize) -> usize {
    *ROOT_ACPI
        .get()
        .unwrap()
        .apic_id_to_cpu_id
        .get(&apic_id)
        .unwrap()
}

pub fn get_apic_id(cpu_id: usize) -> usize {
    *ROOT_ACPI
        .get()
        .unwrap()
        .cpu_id_to_apic_id
        .get(&cpu_id)
        .unwrap()
}
