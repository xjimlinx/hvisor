// ASUS PRIME Z270-AR bare-metal profile.
// Derived from the upstream NUC14MNK x86_64 profile and the machine's
// 2026-09-11 e820/ACPI/PCI inventory. Keep this board local until tested.
use crate::pci_dev;
use crate::{
    arch::zone::HvArchZoneConfig, config::*, memory::GuestPhysAddr, pci::vpci_dev::VpciDevType,
};

pub const MEM_TYPE_RESERVED: u32 = 5;
pub const BOARD_NCPUS: usize = 8;

pub const ROOT_ZONE_DTB_ADDR: u64 = 0;
pub const ROOT_ZONE_BOOT_STACK: GuestPhysAddr = 0x7000;
pub const ROOT_ZONE_ENTRY: u64 = 0x8000;
pub const ROOT_ZONE_KERNEL_ADDR: u64 = 0x500_0000;

// All eight hardware threads belong to Zone0; APs await virtual SIPI.
pub const ROOT_ZONE_CPUS: u64 = 0xff;

const ROOT_ZONE_RSDP_REGION: HvConfigMemoryRegion = HvConfigMemoryRegion {
    mem_type: MEM_TYPE_RAM,
    physical_start: 0x50e_0000,
    virtual_start: 0xe_0000,
    size: 0x2_0000,
};
const ROOT_ZONE_RSDP_REGION_ID: usize = 1;

const ROOT_ZONE_UEFI_REGION: HvConfigMemoryRegion = HvConfigMemoryRegion {
    mem_type: MEM_TYPE_RAM,
    physical_start: 0x1a00_0000,
    virtual_start: 0x1500_0000,
    size: 0x1_0000,
};
const ROOT_ZONE_UEFI_REGION_ID: usize = 3;

const ROOT_ZONE_ACPI_REGION: HvConfigMemoryRegion = HvConfigMemoryRegion {
    mem_type: MEM_TYPE_RAM,
    physical_start: 0x3a30_0000,
    virtual_start: 0x3530_0000,
    size: 0x10_0000,
};
const ROOT_ZONE_ACPI_REGION_ID: usize = 6;

pub const ROOT_ZONE_NAME: &str = "root-linux";
// Keep the real generator path enabled after repairing the guest CPUID
// contract. Keep warnings but disable systemd's source-location/debug flood.
// Do not use nosmp/maxcpus: all assigned APs boot through virtual SIPI.
// Bare-metal Wayland profile, validated with GP102 and Plasma Login.
// Keep HDMI audio isolated until separately tested; preserve INFO in hvisor.
pub const ROOT_ZONE_CMDLINE: &str = "video=vesafb console=tty0 earlycon=efifb nvidia_drm.modeset=1 nvidia_drm.fbdev=1 nmi_watchdog=0 modprobe.blacklist=nouveau module_blacklist=nouveau i2c_i801.disable_features=0x10 panic=0 reboot=pci,cold nointremap no_timer_check efi=noruntime pci=pcie_scan_all,lastbus=0 root=UUID=ccb793fb-bdcf-4b15-911b-b17547f69e92 rw rootwait rd.systemd.gpt_auto=0 systemd.gpt_auto=0 noresume systemd.unit=graphical.target systemd.log_level=warning systemd.log_location=0 systemd.show_status=auto loglevel=4 trace_buf_size=256K trace_event=xhci-hcd:xhci_handle_event,xhci-hcd:xhci_handle_command,xhci-hcd:xhci_setup_device hvisor.gpu=graphics hvisor.zone0=1\0";

pub const ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 28] = [
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x500_0000,
        virtual_start: 0,
        size: 0xe_0000,
    },
    ROOT_ZONE_RSDP_REGION,
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x510_0000,
        virtual_start: 0x10_0000,
        size: 0x14f0_0000,
    },
    ROOT_ZONE_UEFI_REGION,
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x1a01_0000,
        virtual_start: 0x1501_0000,
        size: 0x2f_0000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x1a30_0000,
        virtual_start: 0x1530_0000,
        size: 0x2000_0000,
    },
    ROOT_ZONE_ACPI_REGION,
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x3a40_0000,
        virtual_start: 0x3540_0000,
        size: 0x4a21_0000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xfed0_0000,
        virtual_start: 0xfed0_0000,
        size: 0x1000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x1_0000_0000,
        virtual_start: 0x1_0000_0000,
        size: 0x6_0000_0000,
    },
    HvConfigMemoryRegion {
        // Native e820: System RAM through 0x86effffff, not firmware.
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x7_0000_0000,
        virtual_start: 0x7_0000_0000,
        size: 0x1_6f00_0000,
    },
    // Remaining native low-RAM fragments; avoid adjacent ACPI/NVS holes.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x8461_2000,
        virtual_start: 0x8461_2000,
        size: 0x08c9_b000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_RAM,
        physical_start: 0x8fba_2000,
        virtual_start: 0x8fba_2000,
        size: 0x0005_e000,
    },
    // MEI HECI register window, management Zone0 only.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf34_d000,
        virtual_start: 0xdf34_d000,
        size: 0x1000,
    },
    // SMBus BAR0 is 256 bytes; dedicate its containing page to Zone0.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf34_a000,
        virtual_start: 0xdf34_a000,
        size: 0x1000,
    },
    // PCH HD Audio: both native BAR windows, no PMC/LPC registers.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf34_0000,
        virtual_start: 0xdf34_0000,
        size: 0x4000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf32_0000,
        virtual_start: 0xdf32_0000,
        size: 0x10000,
    },
    // ASMedia USB 3.1, physical 04:00.0, BAR0 including MSI-X table.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf20_0000,
        virtual_start: 0xdf20_0000,
        size: 0x8000,
    },
    // AX210: physical 05:00.0, BAR0 including MSI-X table.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf10_0000,
        virtual_start: 0xdf10_0000,
        size: 0x4000,
    },
    // Intel I219-V 00:1f.6 BAR0, kept at its real MMIO address.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf30_0000,
        virtual_start: 0xdf30_0000,
        size: 0x2_0000,
    },
    // Intel PCH xHCI 00:14.0 BAR0 (captured native resource: 64 KiB).
    // Assign together with its PCI function/VT-d domain and ACPI window.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf33_0000,
        virtual_start: 0xdf33_0000,
        size: 0x1_0000,
    },
    // Intel SATA AHCI 00:17.0 BARs. The x86 PCI BAR auto-mapping path is
    // currently disabled in hvisor, so physical endpoints listed in
    // ROOT_PCI_DEVS otherwise enumerate but their register windows fault in
    // Zone0. Keep the three page-aligned identity mappings explicit until the
    // generic BAR insertion path is restored upstream.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf34_8000,
        virtual_start: 0xdf34_8000,
        size: 0x2000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf34_b000,
        virtual_start: 0xdf34_b000,
        size: 0x1000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf34_c000,
        virtual_start: 0xdf34_c000,
        size: 0x1000,
    },
    // GP102 01:00.0 BAR0/1/3 and HDMI audio 01:00.1 BAR0. Keep the
    // firmware BAR addresses: no PCI bridge or unrelated endpoint is exposed.
    // UC is conservative for initial bring-up, including VRAM apertures.
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xde00_0000,
        virtual_start: 0xde00_0000,
        size: 0x100_0000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xc000_0000,
        virtual_start: 0xc000_0000,
        size: 0x1000_0000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xd000_0000,
        virtual_start: 0xd000_0000,
        size: 0x200_0000,
    },
    HvConfigMemoryRegion {
        mem_type: MEM_TYPE_IO,
        physical_start: 0xdf08_0000,
        virtual_start: 0xdf08_0000,
        size: 0x4000,
    },
    // Generated minimal DSDT has no firmware NVS/OperationRegion dependency.
];

const ROOT_ZONE_CMDLINE_ADDR: GuestPhysAddr = 0x9000;
const ROOT_ZONE_SETUP_ADDR: GuestPhysAddr = 0xa000;
const ROOT_ZONE_VMLINUX_ENTRY_ADDR: GuestPhysAddr = 0x10_0000;
const ROOT_ZONE_SCREEN_BASE_ADDR: GuestPhysAddr = 0x8000_0000;
const ROOT_ZONE_INITRD_ADDR: GuestPhysAddr = 0x6000_0000;
// Updated by baremetal/prepare-current-arch-kernel.py from the exact staged
// intel-ucode + initramfs image before hvisor is built.
const ROOT_ZONE_INITRD_SIZE: usize = 0x00c4ed2b;

pub const IRQ_WAKEUP_VIRTIO_DEVICE: usize = 0x6;
pub const ROOT_ZONE_IRQS_BITMAP: &[BitmapWord] = &get_irqs_bitmap(&[0; 32]);
pub const ROOT_ZONE_IOAPIC_BASE: usize = 0xfec0_0000;
pub const ROOT_ARCH_ZONE_CONFIG: HvArchZoneConfig = HvArchZoneConfig {
    ioapic_base: ROOT_ZONE_IOAPIC_BASE,
    ioapic_size: 0x1000,
    kernel_entry_gpa: ROOT_ZONE_VMLINUX_ENTRY_ADDR,
    cmdline_load_gpa: ROOT_ZONE_CMDLINE_ADDR,
    setup_load_gpa: ROOT_ZONE_SETUP_ADDR,
    initrd_load_gpa: ROOT_ZONE_INITRD_ADDR,
    initrd_size: ROOT_ZONE_INITRD_SIZE,
    rsdp_memory_region_id: ROOT_ZONE_RSDP_REGION_ID,
    acpi_memory_region_id: ROOT_ZONE_ACPI_REGION_ID,
    uefi_memory_region_id: ROOT_ZONE_UEFI_REGION_ID,
    screen_base: ROOT_ZONE_SCREEN_BASE_ADDR,
};

pub const ROOT_PCI_CONFIG: [HvPciConfig; 1] = [HvPciConfig {
    bus_range_begin: 0,
    bus_range_end: 0,
    ecam_base: 0xe000_0000,
    ecam_size: 0x100_000,
    io_base: 0,
    io_size: 0,
    pci_io_base: 0,
    mem32_base: 0,
    mem32_size: 0,
    pci_mem32_base: 0,
    mem64_base: 0,
    mem64_size: 0,
    pci_mem64_base: 0,
    domain: 0,
}];

pub const ROOT_PCI_MAX_BUS: usize = 0;
// I219-V is exposed as a standalone function, so enumeration does not need
// the unassigned LPC function 00:1f.0. VT-d still uses physical BDF 00:1f.6.
// GPU functions share guest slot 00:1a; VT-d uses physical 01:00.0/1.
// Keep the existing NVIDIA blacklist until SSH is available for driver tests.
pub const ROOT_PCI_DEVS: [HvPciDevConfig; 11] = [
    pci_dev!(0, 0, 0x16, 0 => 0, 0x16, 0, VpciDevType::Physical), // MEI
    pci_dev!(0, 0, 0x1f, 4 => 0, 0x1e, 0, VpciDevType::Physical), // SMBus
    pci_dev!(0, 0, 0x1f, 3 => 0, 0x1d, 0, VpciDevType::Physical), // PCH audio
    pci_dev!(0, 4, 0x00, 0 => 0, 0x1c, 0, VpciDevType::Physical), // ASMedia USB
    pci_dev!(0, 5, 0x00, 0 => 0, 0x1b, 0, VpciDevType::Physical), // AX210
    pci_dev!(0, 0, 0x00, 0 => 0, 0x00, 0, VpciDevType::Physical), // host bridge
    pci_dev!(0, 0, 0x14, 0 => 0, 0x14, 0, VpciDevType::Physical), // Intel xHCI
    pci_dev!(0, 0, 0x17, 0 => 0, 0x17, 0, VpciDevType::Physical), // SATA AHCI
    pci_dev!(0, 0, 0x1f, 6 => 0, 0x19, 0, VpciDevType::Physical), // I219-V -> 00:19.0
    pci_dev!(0, 1, 0x00, 0 => 0, 0x1a, 0, VpciDevType::Physical), // GTX 1080 Ti
    pci_dev!(0, 1, 0x00, 1 => 0, 0x1a, 1, VpciDevType::Physical), // HDMI audio
];

#[cfg(all(graphics))]
pub const GRAPHICS_FONT: &[u8] =
    include_bytes!("../../platform/x86_64/qemu/image/font/spleen-6x12.psf");
