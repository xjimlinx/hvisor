#!/usr/bin/env python3
"""Exercise actual next_device_not_ok against highest-child-bus regression."""
from pathlib import Path
import subprocess
repo=Path(__file__).resolve().parents[3]
source=(repo/'src/pci/pci_struct.rs').read_text()
start=source.index('    fn next_device_not_ok(&mut self) -> bool')
end=source.index('    fn next(&mut self, current_bridge:',start)
method=source[start:end]
out=repo/'target/pci-final-bus-tests'
out.mkdir(parents=True,exist_ok=True)
harness='''
const MAX_DEVICE:u8=31;
struct Bridge { has_only_one_child:bool, device:u8, function:u8, is_mulitple_function:bool, subordinate_bus:u8 }
impl Bridge { fn update_bridge_bus(&mut self) {} }
struct Scanner {stack:Vec<Bridge>,is_finish:bool,function:u8,is_mulitple_function:bool,bus_range:std::ops::Range<usize>}
impl Scanner { METHOD }
fn frame(device:u8,subordinate_bus:u8)->Bridge {Bridge{has_only_one_child:false,device,function:0,is_mulitple_function:false,subordinate_bus}}
#[test] fn final_child_does_not_finish_parent() {
 let mut s=Scanner{stack:vec![frame(0x1d,6),frame(31,6)],is_finish:false,function:0,is_mulitple_function:false,bus_range:0..6};
 assert!(s.next_device_not_ok()); assert!(!s.is_finish);
 assert!(!s.next_device_not_ok()); assert_eq!(s.stack[0].device,0x1e);
 assert!(!s.next_device_not_ok()); assert_eq!(s.stack[0].device,0x1f);
}
#[test] fn root_pop_finishes_even_if_subordinate_differs() {
 let mut s=Scanner{stack:vec![frame(31,1)],is_finish:false,function:0,is_mulitple_function:false,bus_range:0..6};
 assert!(s.next_device_not_ok()); assert!(s.is_finish);
}
'''.replace('METHOD',method)
(out/'test.rs').write_text(harness)
subprocess.run(['rustc','--test',str(out/'test.rs'),'-o',str(out/'test')],check=True)
subprocess.run([str(out/'test')],check=True)
