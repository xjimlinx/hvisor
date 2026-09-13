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
        cpu::{this_cpu_id, ArchCpu},
        cpuid::{CpuIdEax, ExtendedFeaturesEcx, FeatureInfoFlags},
        idt::{IdtStruct, IdtVector},
        ipi,
        msr::Msr::{self, *},
        pio::I8042_PORT,
        s2pt::Stage2PageFaultInfo,
        vmcs::*,
        vmx::{VmxCrAccessInfo, VmxExitInfo, VmxExitReason, VmxInterruptInfo, VmxIoExitInfo},
    },
    cpu_data::{this_cpu_data, this_zone},
    device::{
        irqchip::{
            inject_vector,
            pic::{ioapic::irqs, lapic::VirtLocalApic},
        },
        uart::{virt_console_io_read, virt_console_io_write, UartReg},
    },
    error::HvResult,
    hypercall::HyperCall,
    memory::{mmio_handle_access, MMIOAccess, MemFlags},
    zone::this_zone_id,
};
use bit_field::BitField;
use core::mem::size_of;
use x86_64::registers::control::Cr4Flags;

use super::{
    pci::{handle_pci_config_port_read, handle_pci_config_port_write},
    pio::{PCI_CONFIG_ADDR_PORT, PCI_CONFIG_DATA_PORT, UART_COM1_PORT},
};

core::arch::global_asm!(
    include_str!("trap.S"),
    sym arch_handle_trap
);

const IRQ_VECTOR_START: u8 = 0x20;
const IRQ_VECTOR_END: u8 = 0xff;
const NMI_VECTOR: u8 = 2;

const VM_EXIT_INSTR_LEN_CPUID: u8 = 2;
const VM_EXIT_INSTR_LEN_HLT: u8 = 1;
const VM_EXIT_INSTR_LEN_RDMSR: u8 = 2;
const VM_EXIT_INSTR_LEN_WRMSR: u8 = 2;
const VM_EXIT_INSTR_LEN_VMCALL: u8 = 3;

#[cfg(z270_minimal_acpi)]
#[path = "parking_exit.rs"]
mod parking_exit;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct TrapFrame {
    pub usr: [u64; 15],

    // pushed by 'trap.S'
    pub vector: u64,
    pub error_code: u64,

    // pushed by CPU
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

lazy_static::lazy_static! {
    static ref IDT: IdtStruct = IdtStruct::new();
}

pub fn install_trap_vector(cpu_id: usize) {
    super::idt::install_fail_stop_tss(cpu_id);
    IDT.load();
}

#[no_mangle]
pub fn arch_handle_trap(tf: &mut TrapFrame) {
    // println!("trap {} @ {:#x}", tf.vector, tf.rip);
    match tf.vector as u8 {
        // With NMI exiting disabled, NMIs that arrive in VMX non-root mode go
        // directly to Zone0 Linux. An NMI can still land during a short VMX-root
        // window (or while an AP is entering its parking VM). Do not print, take
        // locks, or touch the VMCS from this asynchronous context: returning via
        // IRETQ safely completes the host-side NMI, and a later watchdog sample
        // will reach Linux after guest execution resumes.
        NMI_VECTOR => return,
        IRQ_VECTOR_START..=IRQ_VECTOR_END => handle_irq(tf.vector as u8),
        _ => {
            // Fail closed before formatting the diagnostic. This prevents a
            // nested maskable interrupt from obscuring the original exception
            // or escalating it into a hardware reset.
            unsafe {
                core::arch::asm!("cli", options(nomem, nostack));
            }
            if tf.vector == 8 {
                println!(
                    "DOUBLE FAULT on emergency IST stack (error_code = {:#x})",
                    tf.error_code
                );
            }
            println!(
                "Unhandled exception {} (error_code = {:#x}) @ {:#x}\nRDI={:#x} RSI={:#x} RAX={:#x} RBX={:#x}\nRSP={:#x} RBP={:#x} RFLAGS={:#x}",
                tf.vector,
                tf.error_code,
                tf.rip,
                tf.usr[6],
                tf.usr[5],
                tf.usr[0],
                tf.usr[3],
                tf.rsp,
                tf.usr[4],
                tf.rflags,
            );
            println!("FAIL-STOP: CPU halted; use a manual reset to leave this screen");
            loop {
                unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)) };
            }
        }
    }
}

fn handle_irq(vector: u8) {
    let is_timer = vector == this_cpu_data().arch_cpu.virt_lapic.virt_timer_vector;

    match vector {
        IdtVector::VIRT_IPI_VECTOR if !is_timer => {
            ipi::handle_virt_ipi();
        }
        // 0x21 is not permanently owned by a host keyboard: the guest may
        // allocate it to PCI MSI. Legacy PIC is masked by hvisor already.
        IdtVector::APIC_SPURIOUS_VECTOR | IdtVector::APIC_ERROR_VECTOR if !is_timer => {}
        _ => {
            if vector >= 0x20 && this_cpu_data().vcpu_state.is_running() {
                inject_vector(this_cpu_id(), vector, None, false);
            }
        }
    }
    unsafe { VirtLocalApic::phys_local_apic().end_of_interrupt() };
}

fn handle_cpuid(arch_cpu: &mut ArchCpu) -> HvResult {
    use raw_cpuid::{cpuid, CpuIdResult};
    // Do not advertise ACRN: Linux then replaces native TSC calibration with
    // CPUID.40000010H (kHz), an ACRN ABI that hvisor does not implement.
    let signature = [
        u32::from_le_bytes(*b"hvis"),
        u32::from_le_bytes(*b"orhv"),
        u32::from_le_bytes(*b"isor"),
    ];
    let cr4_flags = Cr4Flags::from_bits_truncate(arch_cpu.cr(4) as _);
    let regs = arch_cpu.regs_mut();
    let rax: Result<CpuIdEax, u32> = (regs.rax as u32).try_into();
    let mut res: CpuIdResult = cpuid!(regs.rax, regs.rcx);

    if (0x4000_0001..0x5000_0000).contains(&(regs.rax as u32)) {
        // Unsupported hypervisor leaves must not fall through to native
        // CPUID, which may return unrelated maximum-leaf data.
        res = CpuIdResult { eax: 0, ebx: 0, ecx: 0, edx: 0 };
    } else if let Ok(function) = rax {
        res = match function {
            CpuIdEax::FeatureInfo => {
                let mut res = cpuid!(regs.rax, regs.rcx);
                let mut ecx = FeatureInfoFlags::from_bits_truncate(res.ecx as _);

                ecx.remove(FeatureInfoFlags::VMX);
                // XSETBV is not virtualized and XCR0 is not switched by this
                // x86 backend. Keep the entire XSAVE-dependent feature family
                // hidden as one coherent CPUID contract. Advertising AVX while
                // hiding XSAVE can make modern libc select code whose register
                // state Linux never enabled.
                ecx.remove(
                    FeatureInfoFlags::XSAVE
                        | FeatureInfoFlags::OSXSAVE
                        | FeatureInfoFlags::AVX
                        | FeatureInfoFlags::FMA
                        | FeatureInfoFlags::F16C,
                );

                ecx.insert(FeatureInfoFlags::X2APIC);
                ecx.insert(FeatureInfoFlags::HYPERVISOR);
                res.ecx = ecx.bits() as _;

                let mut edx = FeatureInfoFlags::from_bits_truncate((res.edx as u64) << 32);
                // edx.remove(FeatureInfoFlags::TSC);
                res.edx = (edx.bits() >> 32) as _;

                res
            }
            CpuIdEax::StructuredExtendedFeatureInfo => {
                let mut res = cpuid!(regs.rax, regs.rcx);
                let mut ecx = ExtendedFeaturesEcx::from_bits_truncate(res.ecx as _);
                ecx.remove(ExtendedFeaturesEcx::WAITPKG);
                res.ecx = ecx.bits() as _;

                if regs.rcx == 0 {
                    // AVX2 and MPX depend on extended state components that
                    // are deliberately hidden above. Kaby Lake has no AVX-512,
                    // so no additional AVX-512 leaf-7 bits need masking here.
                    const AVX2: u32 = 1 << 5;
                    const MPX_BNDREGS: u32 = 1 << 14;
                    const MPX_BNDCSR: u32 = 1 << 15;
                    res.ebx &= !(AVX2 | MPX_BNDREGS | MPX_BNDCSR);
                }

                res
            }
            CpuIdEax::ExtendedStateInfo => CpuIdResult {
                // Leaf 0xd must not describe native XCR0/XSS state after
                // CPUID.1:ECX.XSAVE has been hidden from the guest.
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: 0,
            },
            // The guest reads the physical TSC directly: there is no VMCS TSC
            // offset or scaling.  Preserve the matching hardware ratio,
            // crystal frequency, base/max frequency and 100 MHz bus frequency
            // instead of synthesizing inconsistent CPUID.15H/16H values.
            CpuIdEax::TscInfo | CpuIdEax::ProcessorFrequencyInfo => {
                cpuid!(regs.rax, regs.rcx)
            }
            CpuIdEax::HypervisorInfo => CpuIdResult {
                eax: CpuIdEax::HypervisorInfo as u32,
                ebx: signature[0],
                ecx: signature[1],
                edx: signature[2],
            },
            CpuIdEax::HypervisorFeatures => CpuIdResult {
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: 0,
            },
            _ => cpuid!(regs.rax, regs.rcx),
        };
    }

    trace!(
        "VM exit: CPUID({:#x}, {:#x}): {:?}",
        regs.rax,
        regs.rcx,
        res
    );
    regs.rax = res.eax as _;
    regs.rbx = res.ebx as _;
    regs.rcx = res.ecx as _;
    regs.rdx = res.edx as _;

    arch_cpu.advance_guest_rip(VM_EXIT_INSTR_LEN_CPUID)?;
    Ok(())
}

fn handle_cr_access(arch_cpu: &mut ArchCpu) -> HvResult {
    let cr_access_info = VmxCrAccessInfo::new()?;
    panic!(
        "VM-exit: CR{} access:\n{:#x?}",
        cr_access_info.cr_n, arch_cpu
    );

    match cr_access_info.cr_n {
        0 => {}
        _ => {}
    }

    Ok(())
}

fn handle_external_interrupt() -> HvResult {
    let int_info = VmxInterruptInfo::new()?;
    trace!("VM-exit: external interrupt: {:#x?}", int_info);
    assert!(int_info.valid);
    if int_info.vector == 0x21 {
        // info!("External interrupt: IRQ1 (keyboard)");
    }
    handle_irq(int_info.vector);
    Ok(())
}

fn handle_hypercall(arch_cpu: &mut ArchCpu) -> HvResult {
    let regs = arch_cpu.regs_mut();
    debug!(
        "VM exit: VMCALL({:#x}): {:x?}",
        regs.rax,
        [regs.rdi, regs.rsi]
    );
    let (code, arg0, arg1) = (regs.rax, regs.rdi, regs.rsi);
    let cpu_data = this_cpu_data();
    let result = match HyperCall::new(cpu_data).hypercall(code as _, arg0, arg1) {
        Ok(ret) => ret as _,
        Err(e) => {
            error!("hypercall error: {:#?}", e);
            e.code()
        }
    };
    debug!("HVC result = {}", result);
    regs.rax = result as _;

    arch_cpu.advance_guest_rip(VM_EXIT_INSTR_LEN_VMCALL)?;
    Ok(())
}

fn handle_io_instruction(arch_cpu: &mut ArchCpu, exit_info: &VmxExitInfo) -> HvResult {
    let io_info = VmxIoExitInfo::new()?;

    /*info!(
        "VM exit: I/O instruction @ {:#x}: {:#x?}",
        exit_info.guest_rip, io_info,
    );*/

    if io_info.is_string {
        error!("INS/OUTS instructions are not supported!");
        return hv_result_err!(ENOSYS);
    }
    if io_info.is_repeat {
        error!("REP prefixed I/O instructions are not supported!");
        return hv_result_err!(ENOSYS);
    }

    // CF9 byte accesses are the PCH reset register, not a partial CF8
    // CONFIG_ADDRESS access. Keep dword CF8 accesses virtualized. Only the
    // bare-metal root zone owns platform reset; other zones must not reach it.
    #[cfg(z270_minimal_acpi)]
    if io_info.port == 0xcf9 && io_info.access_size == 1 {
        if this_zone_id() != 0 {
            return hv_result_err!(EPERM);
        }
        let mut port = x86_64::instructions::port::Port::<u8>::new(0xcf9);
        if io_info.is_in {
            let value = unsafe { port.read() };
            let regs = arch_cpu.regs_mut();
            regs.rax = (regs.rax & !0xff) | value as u64;
        } else {
            let value = arch_cpu.regs().rax as u8;
            info!("Zone0 PCH reset control write: {:#x}", value);
            unsafe { port.write(value) };
        }
        arch_cpu.advance_guest_rip(exit_info.exit_instruction_length as _)?;
        return Ok(());
    }

    let mut value: u32 = 0;
    if !io_info.is_in {
        let rax = arch_cpu.regs().rax;
        value = match io_info.access_size {
            1 => rax & 0xff,
            2 => rax & 0xffff,
            4 => rax,
            _ => unreachable!(),
        } as _;

        // TODO: reconstruct
        if PCI_CONFIG_ADDR_PORT.contains(&io_info.port)
            || PCI_CONFIG_DATA_PORT.contains(&io_info.port)
        {
            handle_pci_config_port_write(&io_info, value);
        } else if UART_COM1_PORT.contains(&io_info.port) {
            if this_zone_id() == 0 {
                virt_console_io_write(io_info.port, value);
            } else {
                virt_console_io_write(io_info.port, value);
                // info!("zone1 uart write from {:x}: {:x}", io_info.port, value);
            }
        } else if I8042_PORT.contains(&io_info.port) {
            // info!("unhandled port io write {:x} value: {:x}", io_info.port, value);
        }
    } else {
        if PCI_CONFIG_ADDR_PORT.contains(&io_info.port)
            || PCI_CONFIG_DATA_PORT.contains(&io_info.port)
        {
            value = handle_pci_config_port_read(&io_info);
        } else if UART_COM1_PORT.contains(&io_info.port) {
            if this_zone_id() == 0 {
                value = virt_console_io_read(io_info.port);
            } else {
                value = 0xff;
                value = virt_console_io_read(io_info.port);
                // info!("zone1 uart read from {:x}: {:x}", io_info.port, value);
            }
        } else if I8042_PORT.contains(&io_info.port) {
            value = 0xff;
        } else {
            // info!("unhandled port io read {:x}", io_info.port);
            value = 0x0;
        }
        let rax = &mut arch_cpu.regs_mut().rax;
        // SDM Vol. 1, Section 3.4.1.1:
        // * 32-bit operands generate a 32-bit result, zero-extended to a 64-bit result in the
        //   destination general-purpose register.
        // * 8-bit and 16-bit operands generate an 8-bit or 16-bit result. The upper 56 bits or
        //   48 bits (respectively) of the destination general-purpose register are not modified
        //   by the operation.
        match io_info.access_size {
            1 => *rax = (*rax & !0xff) | (value & 0xff) as u64,
            2 => *rax = (*rax & !0xffff) | (value & 0xffff) as u64,
            4 => *rax = value as u64,
            _ => unreachable!(),
        }
    }

    arch_cpu.advance_guest_rip(exit_info.exit_instruction_length as _)?;
    Ok(())
}

fn handle_msr_read(arch_cpu: &mut ArchCpu) -> HvResult {
    let rcx = arch_cpu.regs().rcx as u32;

    if let Ok(msr) = Msr::try_from(rcx) {
        let res = if msr == IA32_APIC_BASE {
            Ok(arch_cpu.virt_lapic.guest.base)
        } else if VirtLocalApic::msr_range().contains(&rcx) {
            arch_cpu.virt_lapic.rdmsr(msr)
        } else {
            hv_result_err!(ENOSYS)
        };

        if let Ok(value) = res {
            debug!("VM exit: RDMSR({:#x}) -> {:#x}", rcx, value);
            arch_cpu.regs_mut().rax = value & 0xffff_ffff;
            arch_cpu.regs_mut().rdx = value >> 32;
        } else {
            warn!("Failed to handle RDMSR({:#x}): {:?}", rcx, res);
        }
    } else {
        // warn!("Unrecognized RDMSR({:#x})", rcx);
    }

    arch_cpu.advance_guest_rip(VM_EXIT_INSTR_LEN_RDMSR)?;
    Ok(())
}

fn handle_msr_write(arch_cpu: &mut ArchCpu) -> HvResult {
    let rcx = arch_cpu.regs().rcx as u32;
    let msr = Msr::try_from(rcx).unwrap();
    let value = (arch_cpu.regs().rax & 0xffff_ffff) | (arch_cpu.regs().rdx << 32);
    debug!("VM exit: WRMSR({:#x}) <- {:#x}", rcx, value);

    let res = if msr == IA32_APIC_BASE {
        if arch_cpu.virt_lapic.guest.set_base(value) {
            info!("guest APIC mode/base={:#x}", arch_cpu.virt_lapic.guest.base);
            Ok(())
        } else {
            return hv_result_err!(EINVAL);
        }
    } else if VirtLocalApic::msr_range().contains(&rcx) || msr == IA32_TSC_DEADLINE {
        arch_cpu.virt_lapic.wrmsr(msr, value)
    } else {
        hv_result_err!(ENOSYS)
    };

    if res.is_err() {
        warn!(
            "Failed to handle WRMSR({:#x}) <- {:#x}: {:?}\n{:#x?}",
            rcx, value, res, arch_cpu
        );
    }
    arch_cpu.advance_guest_rip(VM_EXIT_INSTR_LEN_WRMSR)?;
    Ok(())
}

fn handle_s2pt_violation(arch_cpu: &mut ArchCpu, exit_info: &VmxExitInfo) -> HvResult {
    let fault_info = Stage2PageFaultInfo::new()?;
    let result = mmio_handle_access(&mut MMIOAccess {
        address: fault_info.fault_guest_paddr,
        size: 0,
        is_write: fault_info.access_flags.contains(MemFlags::WRITE),
        value: 0,
    });
    if let Err(error) = result {
        // Keep the fault address visible instead of letting the caller's full
        // ArchCpu dump scroll the useful MMIO diagnostic off the screen.
        panic!(
            "EPT/MMIO FAILURE\nGPA={:#x} RIP={:#x}\naccess={:?} qualification={:#x}\nerror={:?}",
            fault_info.fault_guest_paddr,
            exit_info.guest_rip,
            fault_info.access_flags,
            VmcsReadOnlyNW::EXIT_QUALIFICATION.read().unwrap_or(0),
            error
        );
    }

    Ok(())
}

fn handle_triple_fault(arch_cpu: &mut ArchCpu, exit_info: &VmxExitInfo) -> HvResult {
    panic!(
        "VM exit: Triple fault @ {:#x}, instr length: {:x}\n {:#x?}",
        exit_info.guest_rip, exit_info.exit_instruction_length, arch_cpu
    );
    // arch_cpu.advance_guest_rip(exit_info.exit_instruction_length as _)?;
    Ok(())
}

pub fn handle_vmexit(arch_cpu: &mut ArchCpu) -> HvResult {
    let exit_info = VmxExitInfo::new()?;
    debug!("VM exit: {:#x?}", exit_info);

    if exit_info.entry_failure {
        panic!("VM entry failed: {:#x?}", exit_info);
    }

    #[cfg(z270_minimal_acpi)]
    if parking_exit::resume_parked_init(
        exit_info.exit_reason as u32,
        this_cpu_id(),
        crate::platform::ROOT_ZONE_CPUS,
        this_cpu_data().vcpu_state.is_running(),
    ) {
        // INIT caused an exit, not execution of an instruction or a guest
        // reset. Resume the existing parking loop without advancing RIP,
        // injecting INIT into Linux, or changing VMCS activity/state.
        // Never apply this policy to the BSP or an assigned/running vCPU.
        use core::sync::atomic::{AtomicU64, Ordering};
        static REPORTED: AtomicU64 = AtomicU64::new(0);
        let bit = 1u64 << this_cpu_id();
        if REPORTED.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
            warn!("CPU{}: physical INIT received in parking VM; preserving parked state (source unknown)", this_cpu_id());
        }
        return Ok(());
    }

    let res = match exit_info.exit_reason {
        VmxExitReason::EXTERNAL_INTERRUPT => handle_external_interrupt(),
        VmxExitReason::TRIPLE_FAULT => handle_triple_fault(arch_cpu, &exit_info),
        VmxExitReason::INTERRUPT_WINDOW => Vmcs::set_interrupt_window(false),
        VmxExitReason::CPUID => handle_cpuid(arch_cpu),
        VmxExitReason::HLT => {
            arch_cpu.advance_guest_rip(VM_EXIT_INSTR_LEN_HLT)?;
            Ok(())
        }
        VmxExitReason::VMCALL => handle_hypercall(arch_cpu),
        VmxExitReason::CR_ACCESS => handle_cr_access(arch_cpu),
        VmxExitReason::IO_INSTRUCTION => handle_io_instruction(arch_cpu, &exit_info),
        VmxExitReason::MSR_READ => handle_msr_read(arch_cpu),
        VmxExitReason::MSR_WRITE => handle_msr_write(arch_cpu),
        VmxExitReason::EPT_VIOLATION => handle_s2pt_violation(arch_cpu, &exit_info),
        _ => panic!(
            "Unhandled VM-Exit CPU{} reason={:?} raw={:#x} running={} RIP={:#x} RSP={:#x} qualification={:#x} intr={:#x} vectoring={:#x}",
            this_cpu_id(), exit_info.exit_reason,
            VmcsReadOnly32::EXIT_REASON.read().unwrap_or(u32::MAX),
            this_cpu_data().vcpu_state.is_running(), exit_info.guest_rip,
            VmcsGuestNW::RSP.read().unwrap_or(usize::MAX),
            VmcsReadOnlyNW::EXIT_QUALIFICATION.read().unwrap_or(usize::MAX),
            VmcsReadOnly32::VMEXIT_INTERRUPTION_INFO.read().unwrap_or(u32::MAX),
            VmcsReadOnly32::IDT_VECTORING_INFO.read().unwrap_or(u32::MAX)
        ),
    };

    if res.is_err() {
        panic!(
            "Failed to handle VM-exit {:?}:\n{:#x?}\n{:?}",
            exit_info.exit_reason,
            arch_cpu,
            res.err()
        );
    }

    #[cfg(intel_vtd)]
    crate::device::iommu::check_faults();

    Ok(())
}
