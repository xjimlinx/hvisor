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

use crate::{consts::MAX_CPU_NUM, error::HvResult, zone::this_zone_id};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use core::{mem::size_of, u32};
use spin::{Mutex, Once};
use x86_64::{
    instructions::tables::{lgdt, load_tss},
    structures::{
        gdt::SegmentSelector,
        idt::{Entry, HandlerFunc, InterruptDescriptorTable},
        tss::TaskStateSegment,
        DescriptorTablePointer,
    },
    PrivilegeLevel, VirtAddr,
};

const VECTOR_CNT: usize = 256;
const DOUBLE_FAULT_VECTOR: usize = 8;
const FAIL_STOP_IST_INDEX: u16 = 0;
const FAIL_STOP_STACK_SIZE: usize = 16 * 1024;
const TSS_SELECTOR_INDEX: u16 = 4;

const GDT_NULL: u64 = 0x0000_0000_0000_0000;
const GDT_CODE32: u64 = 0x00cf_9b00_0000_ffff;
const GDT_CODE64: u64 = 0x00af_9b00_0000_ffff;
const GDT_DATA32: u64 = 0x00cf_9300_0000_ffff;

#[repr(align(16))]
struct FailStopStacks([[u8; FAIL_STOP_STACK_SIZE]; MAX_CPU_NUM]);

// These tables are initialized after BSS clearing and before the IDT is
// loaded. They are per-CPU because loading a TSS marks its descriptor busy.
static mut FAIL_STOP_STACKS: FailStopStacks =
    FailStopStacks([[0; FAIL_STOP_STACK_SIZE]; MAX_CPU_NUM]);
static mut FAIL_STOP_TSS: [TaskStateSegment; MAX_CPU_NUM] =
    [TaskStateSegment::new(); MAX_CPU_NUM];
static mut FAIL_STOP_GDTS: [[u64; 6]; MAX_CPU_NUM] = [[0; 6]; MAX_CPU_NUM];

#[allow(non_snake_case)]
pub mod IdtVector {
    pub const I8042_KEYBOARD_VECTOR: u8 = 0x21;
    pub const VIRT_IPI_VECTOR: u8 = 0xef;
    pub const APIC_ERROR_VECTOR: u8 = 0xfc;
    pub const APIC_SPURIOUS_VECTOR: u8 = 0xfd;
    pub const APIC_TIMER_VECTOR: u8 = 0xfe;
}

pub struct IdtStruct {
    table: InterruptDescriptorTable,
}

impl IdtStruct {
    pub fn new() -> Self {
        extern "C" {
            #[link_name = "_hyp_trap_vector"]
            static ENTRIES: [extern "C" fn(); VECTOR_CNT];
        }
        let mut idt = Self {
            table: InterruptDescriptorTable::new(),
        };
        let entries = unsafe {
            core::slice::from_raw_parts_mut(
                &mut idt.table as *mut _ as *mut Entry<HandlerFunc>,
                VECTOR_CNT,
            )
        };
        for i in 0..VECTOR_CNT {
            let options =
                entries[i].set_handler_fn(unsafe { core::mem::transmute(ENTRIES[i]) });
            if i == DOUBLE_FAULT_VECTOR {
                // A double fault must not use the possibly corrupted regular
                // stack. Hardware IST index 1 is software index 0 here.
                unsafe {
                    options.set_stack_index(FAIL_STOP_IST_INDEX);
                }
            }
        }
        idt
    }

    pub fn load(&'static self) {
        self.table.load();
    }
}

/// Install a per-CPU TSS/GDT whose first IST entry is a dedicated emergency
/// stack. This makes a double fault observable instead of allowing it to
/// escalate immediately into a reset-causing triple fault.
pub fn install_fail_stop_tss(cpu_id: usize) {
    assert!(cpu_id < MAX_CPU_NUM);

    unsafe {
        let stack_ptr = core::ptr::addr_of_mut!(FAIL_STOP_STACKS.0)
            .cast::<[u8; FAIL_STOP_STACK_SIZE]>()
            .add(cpu_id);
        let stack_top = stack_ptr.cast::<u8>().add(FAIL_STOP_STACK_SIZE) as u64;

        let tss_ptr = core::ptr::addr_of_mut!(FAIL_STOP_TSS)
            .cast::<TaskStateSegment>()
            .add(cpu_id);
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[FAIL_STOP_IST_INDEX as usize] = VirtAddr::new(stack_top);
        tss_ptr.write(tss);

        // Keep the existing selector layout (64-bit CS remains 0x10), but
        // replace the hard-coded TSS descriptor with this build's real TSS.
        let tss_addr = tss_ptr as u64;
        let tss_limit = (size_of::<TaskStateSegment>() - 1) as u64;
        let tss_low = (tss_limit & 0xffff)
            | ((tss_addr & 0xffff) << 16)
            | (((tss_addr >> 16) & 0xff) << 32)
            | (0x89u64 << 40)
            | (((tss_limit >> 16) & 0xf) << 48)
            | (((tss_addr >> 24) & 0xff) << 56);
        let tss_high = tss_addr >> 32;

        let gdt_ptr = core::ptr::addr_of_mut!(FAIL_STOP_GDTS)
            .cast::<[u64; 6]>()
            .add(cpu_id);
        gdt_ptr.write([
            GDT_NULL,
            GDT_CODE32,
            GDT_CODE64,
            GDT_DATA32,
            tss_low,
            tss_high,
        ]);

        let gdtr = DescriptorTablePointer {
            limit: (size_of::<[u64; 6]>() - 1) as u16,
            base: VirtAddr::new(gdt_ptr as u64),
        };
        lgdt(&gdtr);
        load_tss(SegmentSelector::new(
            TSS_SELECTOR_INDEX,
            PrivilegeLevel::Ring0,
        ));
    }
}
