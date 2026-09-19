// Copyright (c) 2025 Syswonder
// hvisor is licensed under Mulan PSL v2.
//
// Compatibility board for the Baozixu OK8MP-C Zone-0 DTB.  The ownership
// layout is retained from the historical profile while using current Hvisor
// configuration types and GICv3 representation.
use crate::{
    arch::{
        mmu::MemoryType,
        zone::{GicConfig, Gicv3Config, HvArchZoneConfig},
    },
    config::*,
};

pub const BOARD_NAME: &str = "forlinx-ok8mpc-core";
pub const BOARD_NCPUS: usize = 4;
pub const BOARD_UART_BASE: u64 = 0x30890000;

#[rustfmt::skip]
pub static BOARD_MPIDR_MAPPINGS: [u64; BOARD_NCPUS] = [0x0, 0x1, 0x2, 0x3];

#[rustfmt::skip]
pub const BOARD_PHYSMEM_LIST: &[(u64, u64, MemoryType)] = &[
    (0x0,        0x40000000,  MemoryType::Device),
    (0x40000000, 0x100000000, MemoryType::Normal),
];

pub const ROOT_ZONE_DTB_ADDR: u64 = 0xa0000000;
pub const ROOT_ZONE_KERNEL_ADDR: u64 = 0xa0400000;
pub const ROOT_ZONE_ENTRY: u64 = 0xa0400000;
pub const ROOT_ZONE_CPUS: u64 = (1 << 0) | (1 << 1);
pub const ROOT_ZONE_NAME: &str = "root-linux";

/// Historical Baozixu ownership list plus AIPS2.  Linux 7.2 accesses its
/// system counter at 0x306b002c early, so AIPS2 must be present on current
/// kernels even though the old board list omitted it.
#[rustfmt::skip]
pub const ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 13] = [
    HvConfigMemoryRegion { mem_type: MEM_TYPE_RAM, physical_start: 0x50000000, virtual_start: 0x50000000, size: 0x80000000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x30000000, virtual_start: 0x30000000, size: 0x400000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x30400000, virtual_start: 0x30400000, size: 0x400000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x30800000, virtual_start: 0x30800000, size: 0x400000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x30c00000, virtual_start: 0x30c00000, size: 0x400000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x32c00000, virtual_start: 0x32c00000, size: 0x400000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x38000000, virtual_start: 0x38000000, size: 0x8000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x38008000, virtual_start: 0x38008000, size: 0x8000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x38100000, virtual_start: 0x38100000, size: 0x10000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x381f0000, virtual_start: 0x381f0000, size: 0x1000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x38200000, virtual_start: 0x38200000, size: 0x10000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x382f0000, virtual_start: 0x382f0000, size: 0x1000 },
    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: 0x38500000, virtual_start: 0x38500000, size: 0x20000 },
];

pub const IRQ_WAKEUP_VIRTIO_DEVICE: usize = 32 + 0x20;

/// Exact 36-SPI list in Baozixu's i.MX8MP board profile.
#[rustfmt::skip]
pub const ROOT_ZONE_IRQS_BITMAP: &[BitmapWord] = &get_irqs_bitmap(&[
    34, 35, 36, 37, 38, 45, 52, 54, 55, 56, 57, 58, 59, 64, 67, 72, 73, 74,
    75, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 135, 150, 151, 152,
    162, 180, 181,
]);

pub const ROOT_ARCH_ZONE_CONFIG: HvArchZoneConfig = HvArchZoneConfig {
    is_aarch32: 0,
    gic_config: GicConfig::Gicv3(Gicv3Config {
        gicd_base: 0x38800000,
        gicd_size: 0x10000,
        gicr_base: 0x38880000,
        gicr_size: 0xc0000,
        gits_base: 0,
        gits_size: 0,
    }),
};

pub const ROOT_ZONE_IVC_CONFIG: [HvIvcConfig; 0] = [];
