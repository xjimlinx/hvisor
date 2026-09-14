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
//

use crate::error::HvResult;
use crate::pci::pci_struct::{ArcRwLockVirtualPciConfigSpace, Bdf, VirtualPciConfigSpace};
use crate::pci::PciConfigAddress;

macro_rules! pci_virt_log {
    ($($arg:tt)*) => {
        // info!($($arg)*);
        // To switch to debug level, change the line above to:
        debug!($($arg)*);
    };
}

macro_rules! arc_rwlock {
    ($val:expr) => {
        Arc::new(RwLock::new($val))
    };
}

mod blk;
mod pch;
mod rng;
pub mod standard;
pub mod tools;
mod virtio_cap;
mod virtio_queue;
/*
 * PciConfigAccessStatus is used to return the result of the config space access
 * Done(usize): the value is returned in usize
 * Default: use default config space value
 * Reject: the access is rejected
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PciConfigAccessStatus {
    Done(usize),
    Default,
    Reject,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(4))]
pub enum VpciDevType {
    #[default]
    Physical = 0,
    StandardVdev = 1,
    VirtioRng = 2,
    VirtioBlk = 3,
    /// Identity-only Intel PCH ISA bridge used by the Z270 iGPU guest.
    /// It exposes the SPT LPC ID so i915 can select the correct HPD/PCH
    /// register layout without sharing the host's LPC device or DMA.
    PchStub = 4,
    // Add new device types here
}

pub trait VpciDeviceHandler: Sync + Send {
    fn read_cfg(
        &self,
        _dev: ArcRwLockVirtualPciConfigSpace,
        _offset: PciConfigAddress,
        _size: usize,
    ) -> HvResult<PciConfigAccessStatus> {
        return Ok(PciConfigAccessStatus::Default);
    }
    fn write_cfg(
        &self,
        _dev: ArcRwLockVirtualPciConfigSpace,
        _offset: PciConfigAddress,
        _size: usize,
        _value: usize,
    ) -> HvResult<PciConfigAccessStatus> {
        return Ok(PciConfigAccessStatus::Default);
    }
    fn vdev_init(&self, dev: VirtualPciConfigSpace) -> VirtualPciConfigSpace;
}

/*
 * Static handler instances for each device type (except Physical).
 * To add a new device type:
 * 1. Add the variant to VpciDevType enum above
 * 2. Add the handler registration here: (&module::HANDLER, VpciDevType::YourType)
 */
static HANDLERS: &[(&dyn VpciDeviceHandler, VpciDevType)] = &[
    (&standard::HANDLER, VpciDevType::StandardVdev),
    (&rng::HANDLER, VpciDevType::VirtioRng),
    (&blk::HANDLER, VpciDevType::VirtioBlk),
    (&pch::HANDLER, VpciDevType::PchStub),
];

pub(crate) fn get_handler(dev_type: VpciDevType) -> Option<&'static dyn VpciDeviceHandler> {
    HANDLERS
        .iter()
        .find(|(_, ty)| *ty == dev_type)
        .map(|(handler, _)| *handler)
}

#[allow(unused_variables)]
pub(super) fn virt_dev_init(
    bdf: Bdf,
    base: PciConfigAddress,
    dev_type: VpciDevType,
) -> Option<VirtualPciConfigSpace> {
    // Identity-only virtual devices (such as the Z270 PCH stub) do not depend
    // on the optional virtio-pci backend. Keep construction available for all
    // PCI builds; virtio handlers remain gated internally by their features.
    use crate::pci::{pci_access::Bar, pci_struct::ConfigValue};
    let initial_dev = VirtualPciConfigSpace::virt_dev_init_default(
        bdf,
        base,
        dev_type,
        ConfigValue::default(),
        Bar::default(),
        None,
    );

    match dev_type {
        VpciDevType::Physical => {
            warn!("virt_dev_init: physical device is not supported");
            Some(initial_dev)
        }
        _ => {
            if let Some(handler) = get_handler(dev_type) {
                Some(handler.vdev_init(initial_dev))
            } else {
                warn!("virt_dev_init: unknown device type");
                Some(initial_dev)
            }
        }
    }
}
