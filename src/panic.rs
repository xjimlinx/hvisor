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
#[cfg(test)]
use crate::tests::*;

use core::panic::PanicInfo;

#[panic_handler]

fn on_panic(info: &PanicInfo) -> ! {
    // A panic in the bare-metal hypervisor must preserve the last screen for
    // diagnosis. Disable maskable interrupts before touching the logger so a
    // nested timer/device interrupt cannot turn this into a reset path.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    error!("panic occurred: {:#?}", info);
    #[cfg(test)]
    {
        error!("panic occurred when running cargo test, quitting qemu");
        crate::tests::quit_qemu(HvUnitTestResult::Failed);
    }
    #[cfg(target_arch = "x86_64")]
    error!("FAIL-STOP: CPU halted; use a manual reset to leave this screen");

    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack));
        }

        #[cfg(not(target_arch = "x86_64"))]
        core::hint::spin_loop();
    }
}
