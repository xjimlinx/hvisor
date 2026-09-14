//! Minimal, non-DMA PCI identity device for a passthrough Intel display.
//!
//! Kaby Lake i915 needs an Intel ISA bridge ID to select the Sunrise Point
//! PCH hotplug implementation.  Passing the host LPC function through would
//! give the guest ownership of a device used by zone0, so this stub exposes
//! only the standard PCI identity/class fields and no BARs or interrupts.

use super::{PciConfigAccessStatus, VpciDeviceHandler};
use crate::error::HvResult;
use crate::pci::pci_access::{BaseClass, DeviceId, DeviceRevision, Interface, SubClass, VendorId};
use crate::pci::pci_struct::{ArcRwLockVirtualPciConfigSpace, VirtualPciConfigSpace};
use crate::pci::PciConfigAddress;

pub struct PchStubHandler;

impl VpciDeviceHandler for PchStubHandler {
    fn read_cfg(
        &self,
        _dev: ArcRwLockVirtualPciConfigSpace,
        _offset: PciConfigAddress,
        _size: usize,
    ) -> HvResult<PciConfigAccessStatus> {
        Ok(PciConfigAccessStatus::Default)
    }

    fn write_cfg(
        &self,
        _dev: ArcRwLockVirtualPciConfigSpace,
        _offset: PciConfigAddress,
        _size: usize,
        _value: usize,
    ) -> HvResult<PciConfigAccessStatus> {
        // Keep command/status and all resources inert: this device is an
        // identity hint only and must not perform I/O or DMA.
        Ok(PciConfigAccessStatus::Done(0))
    }

    fn vdev_init(&self, mut dev: VirtualPciConfigSpace) -> VirtualPciConfigSpace {
        // 8086:a2c5 is the Sunrise Point-H LPC controller used by Z270.
        let id: (DeviceId, VendorId) = (0xa2c5, 0x8086);
        let class: (BaseClass, SubClass, Interface, DeviceRevision) = (0x06, 0x01, 0x00, 0x00);
        dev.with_config_value_mut(|value| {
            value.set_id(id);
            value.set_class_and_revision_id(class);
        });
        dev
    }
}

pub const HANDLER: PchStubHandler = PchStubHandler;
