#!/usr/bin/env python3
"""Source-order regression checks, NOT proof of hardware reset correctness."""
from pathlib import Path

root = Path(__file__).resolve().parents[3]
cpu = (root / 'src/arch/x86_64/cpu.rs').read_text()
hypercall = (root / 'src/hypercall/mod.rs').read_text()
idle = cpu.split('pub fn idle(', 1)[1].split('/// Guest general-purpose', 1)[0]
assert idle.index('self.setup_vmcs') < idle.index('PARKING_MEMORY_SET.get().unwrap().activate()')
assert idle.index('PARKING_MEMORY_SET.get().unwrap().activate()') < idle.index('vcpu_state.store(VcpuState::Stopped)')
vmcs = cpu.split('fn setup_vmcs(', 1)[1].split('fn setup_vmcs_control', 1)[0]
assert vmcs.index('Vmcs::clear(previous)?') < vmcs.index('self.vmcs_region =')
stop = hypercall.split('fn hv_zone_shutdown', 1)[1].split('fn hv_zone_list', 1)[0]
assert 'zone.write()' not in stop
assert stop.index('drop(map_irq)') < stop.index('let wait_start')
assert stop.index('zone stop timed out; resources retained') < stop.index('zone = None')
assert 'return false' not in stop
assert '5_000_000_000' in stop
assert 'ZONE_LIFECYCLE.try_lock()' in stop
start = hypercall.split('pub fn hv_zone_start', 1)[1].split('fn hv_zone_shutdown', 1)[0]
assert start.index('ZONE_LIFECYCLE.try_lock()') < start.index('seal_for_start')
print('PASS: stop timeout/resource retention, parked publication, VMCS retirement, lifecycle lock order')
