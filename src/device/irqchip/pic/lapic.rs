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
    arch::{
        cpu::{this_apic_id, this_cpu_id},
        idt::IdtVector,
        ipi,
        msr::Msr::{self, *},
    },
    cpu_data::this_cpu_data,
    device::irqchip::pic::pop_vector,
    error::HvResult,
    memory::Frame,
};
use bit_field::BitField;
use core::{ops::Range, u32};
use x2apic::lapic::{LocalApic, LocalApicBuilder};
use x86::msr::wrmsr;

pub struct VirtLocalApic {
    pub phys_lapic: LocalApic,
    pub virt_timer_vector: u8,
    virt_lvt_timer_bits: u32,
    pub guest: super::apic_registers::ApicRegisters,
}

impl VirtLocalApic {
    pub fn new() -> Self {
        Self {
            phys_lapic: Self::new_phys_lapic(
                IdtVector::APIC_TIMER_VECTOR as _,
                IdtVector::APIC_ERROR_VECTOR as _,
                IdtVector::APIC_SPURIOUS_VECTOR as _,
            ),
            virt_timer_vector: IdtVector::APIC_TIMER_VECTOR as _,
            virt_lvt_timer_bits: (1 << 16) as _, // masked
            guest: super::apic_registers::ApicRegisters::new(this_cpu_id() == 0),
        }
    }

    fn new_phys_lapic(timer: usize, error: usize, spurious: usize) -> LocalApic {
        let mut lapic = LocalApicBuilder::new()
            .timer_vector(timer)
            .error_vector(error)
            .spurious_vector(spurious)
            .build()
            .unwrap();
        unsafe {
            lapic.enable();
            lapic.disable_timer();
        }
        lapic
    }

    pub const fn msr_range() -> Range<u32> {
        0x800..0x840
    }

    pub fn phys_local_apic<'a>() -> &'a mut LocalApic {
        &mut this_cpu_data().arch_cpu.virt_lapic.phys_lapic
    }

    pub fn rdmsr(&mut self, msr: Msr) -> HvResult<u64> {
        match msr {
            IA32_X2APIC_ICR => Ok(self.guest.icr(self.guest.regs[0x30])),
            IA32_X2APIC_APICID => {
                // info!("apicid: {:x}", this_cpu_id());
                Ok(this_apic_id() as u64)
            }
            IA32_X2APIC_LDR => Ok(super::apic_destination::logical_id(this_apic_id() as u32) as u64),
            IA32_X2APIC_ISR0 | IA32_X2APIC_ISR1 | IA32_X2APIC_ISR2 | IA32_X2APIC_ISR3
            | IA32_X2APIC_ISR4 | IA32_X2APIC_ISR5 | IA32_X2APIC_ISR6 | IA32_X2APIC_ISR7 => {
                // info!("isr!");
                Ok(super::apic_bitmap(this_cpu_id(), (msr as usize) - 0x810, true) as u64)
            }
            IA32_X2APIC_IRR0 | IA32_X2APIC_IRR1 | IA32_X2APIC_IRR2 | IA32_X2APIC_IRR3
            | IA32_X2APIC_IRR4 | IA32_X2APIC_IRR5 | IA32_X2APIC_IRR6 | IA32_X2APIC_IRR7 => {
                // info!("irr!");
                Ok(super::apic_bitmap(this_cpu_id(), (msr as usize) - 0x820, false) as u64)
            }
            IA32_X2APIC_LVT_TIMER => Ok(self.virt_lvt_timer_bits as _),
            _ => self.read_register((msr as usize - 0x800) << 4).map(|v| v as u64),
        }
    }

    pub fn wrmsr(&mut self, msr: Msr, value: u64) -> HvResult {
        match msr {
            IA32_TSC_DEADLINE => {
                unsafe { wrmsr(msr as u32, value); }
                Ok(())
            }
            IA32_X2APIC_EOI => {
                pop_vector(this_cpu_id());
                Ok(())
            }
            IA32_X2APIC_ICR => {
                // info!("ICR value: {:x}", value);
                self.guest.regs[0x30] = value as u32;
                self.guest.regs[0x31] = ((value >> 32) as u32) << 24;
                ipi::send_ipi(value)?;
                Ok(())
            }
            IA32_X2APIC_LVT_TIMER => {
                let value = value & 0xffff_ffff;
                let vector = value.get_bits(0..=7);
                self.virt_lvt_timer_bits = value as u32;
                self.virt_timer_vector = vector as u8;
                unsafe {
                    wrmsr(IA32_X2APIC_LVT_TIMER as u32, value);
                }
                Ok(())
            }
            _ => self.write_register((msr as usize - 0x800) << 4, value as u32),
        }
    }

    pub fn read_register(&mut self, offset: usize) -> HvResult<u32> {
        match offset {
            0x20 => Ok((this_apic_id() as u32) << 24),
            0x30 => Ok(0x00050014), // xAPIC version 0x14, LVT through error
            0x80 | 0xd0 | 0xe0 | 0xf0 | 0x280 | 0x2f0 | 0x300 | 0x310
            | 0x330..=0x380 | 0x3e0 => Ok(self.guest.regs[offset >> 4]),
            0xa0 => {
                let mut priority = self.guest.regs[8] & 0xf0;
                for bank in 0..8 {
                    let bits = super::apic_bitmap(this_cpu_id(), bank, true);
                    if bits != 0 { priority = priority.max(((bank as u32 * 32) + 31 - bits.leading_zeros()) & 0xf0); }
                }
                Ok(priority)
            }
            0x100..=0x170 => Ok(super::apic_bitmap(this_cpu_id(), (offset - 0x100) >> 4, true)),
            0x180..=0x1f0 => Ok(0), // current pending queue represents edge injections
            0x200..=0x270 => Ok(super::apic_bitmap(this_cpu_id(), (offset - 0x200) >> 4, false)),
            0x320 => Ok(self.virt_lvt_timer_bits),
            0x390 => Ok(unsafe { x86::msr::rdmsr(0x839) } as u32),
            _ => hv_result_err!(EINVAL),
        }
    }

    pub fn write_register(&mut self, offset: usize, value: u32) -> HvResult {
        match offset {
            0xb0 => { pop_vector(this_cpu_id()); }
            0x80 => { self.guest.regs[8] = value & 0xff; }
            0xd0 => { self.guest.regs[0xd] = value & 0xff000000; }
            0xe0 => { self.guest.regs[0xe] = value | 0x0fffffff; }
            0xf0 => { self.guest.regs[0xf] = value & 0x13ff; }
            0x280 => { self.guest.regs[0x28] = 0; }
            0x310 => { self.guest.regs[0x31] = value & 0xff000000; }
            0x300 => {
                self.guest.regs[0x30] = value & !(1 << 12);
                let mut icr = self.guest.icr(value);
                if value & (1 << 11) != 0 && value & (3 << 18) == 0 {
                    // Z270 single-vCPU logical xAPIC destination: convert to
                    // physical before calling the x2APIC IPI dispatcher.
                    if crate::cpu_data::this_zone().cpu_set().iter().count() != 1 {
                        return hv_result_err!(ENOSYS);
                    }
                    let dest = self.guest.regs[0x31] >> 24;
                    if dest & (self.guest.regs[0xd] >> 24) == 0 { return Ok(()); }
                    icr = ((this_apic_id() as u64) << 32) | (value as u64 & !(1 << 11));
                }
                ipi::send_ipi(icr)?;
            }
            0x320 => { return self.wrmsr(IA32_X2APIC_LVT_TIMER, value as u64); }
            0x380 | 0x3e0 => {
                self.guest.regs[offset >> 4] = value;
                unsafe { wrmsr(0x800 + (offset >> 4) as u32, value as u64); }
            }
            0x2f0 | 0x330..=0x370 => {
                // Do not reprogram host performance/NMI/error interrupt routes.
                self.guest.regs[offset >> 4] = value;
            }
            _ => return hv_result_err!(EINVAL),
        }
        Ok(())
    }
}

impl crate::zone::Zone {
    pub fn lapic_mmio_init(&mut self) {
        self.write().mmio_region_register(0xfee00000, 0x1000, lapic_mmio, 0xfee00000);
    }
}

fn lapic_mmio(mmio: &mut crate::memory::MMIOAccess, _: usize) -> HvResult {
    if super::apic_registers::ApicRegisters::index(mmio.address, mmio.size).is_none() {
        return hv_result_err!(EINVAL);
    }
    let lapic = &mut this_cpu_data().arch_cpu.virt_lapic;
    if lapic.guest.base & 0xc00 != 0x800 { return hv_result_err!(EINVAL); }
    if mmio.is_write {
        lapic.write_register(mmio.address, mmio.value as u32)
    } else {
        mmio.value = lapic.read_register(mmio.address)? as usize;
        Ok(())
    }
}
