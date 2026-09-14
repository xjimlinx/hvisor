import copy
import importlib.util
import struct
import shutil
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('boardgen', HERE/'boardgen.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
PROFILE = HERE.parent.parent/'platform/x86_64/z270'


def table(signature, payload):
    data = bytearray(36)+payload
    data[:4] = signature.encode()
    struct.pack_into('<I', data, 4, len(data))
    data[9] = (-sum(data)) & 255
    return data.hex()


def fixture():
    source = (PROFILE/'board.rs').read_text()
    asl = (PROFILE/'minimal-dsdt.asl').read_text()
    ram = [[r['physical_start'],r['physical_start']+r['size']]
           for r in b.memory_regions(source) if r['kind']=='MEM_TYPE_RAM']
    hw = dict(schema=1, native_capture=True, machine='x86_64', dmi={'board_name':'SYNTHETIC TEST ONLY'},
        cpus=[dict(processor=i, apicid=i) for i in range(8)], ram=ram,
        pci=[dict(bdf='0000:00:00.0',vendor='0x8086',device='0x591f',pci_class='0x060000',
                  group=0,parent=None,resources=[],driver=None),
             dict(bdf='0000:00:14.0',vendor='0x8086',device='0xa2af',pci_class='0x0c0330',
                  group=2,parent=None,driver='xhci_hcd',
                  resources=[dict(index=0,start=0xdf330000,end=0xdf340000,flags=0x200)])],
        acpi={'MCFG':table('MCFG',bytearray(8)+struct.pack('<QHBBI',0xe0000000,0,0,255,0)),
              'DMAR':table('DMAR',bytearray(12)+struct.pack('<HHBBHQ',0,16,1,0,0,0xfed90000))})
    policy = dict(profile='intel-trusted-zone0-v1',reviewed=True,
        board_sha256=b.digest(source.encode()),acpi_sha256=b.digest(asl.encode()),dmi=hw['dmi'],
        snapshot_sha256=b.digest(b.dump(hw).encode()),hypervisor_reserved=[0x200000,0x47c8000],
        approved_extra_bar_pages=[])
    return hw,policy,source,asl


class GeneratorTests(unittest.TestCase):
    def setUp(self):
        self.hw,self.policy,self.source,self.asl = fixture()

    def run_generate(self):
        # Simulate an explicit review of the modified test snapshot.
        self.policy['snapshot_sha256'] = b.digest(b.dump(self.hw).encode())
        return b.generate(self.hw,self.policy,self.source,self.asl)

    def test_success_and_identity(self):
        result = self.run_generate()
        self.assertIn('ROOT_PCI_DEVS: [HvPciDevConfig; 2]', result)
        self.assertIn('0, 0x14, 0 => 0, 0x14, 0', result)
        self.assertIn('ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 30]', result)

    def test_unreviewed(self):
        self.policy['reviewed'] = False
        with self.assertRaisesRegex(ValueError,'unreviewed'): self.run_generate()

    def test_guest_capture_rejected(self):
        self.hw['native_capture'] = False
        with self.assertRaisesRegex(ValueError,'native'): self.run_generate()

    def test_template_drift(self):
        self.source += '\n'
        with self.assertRaisesRegex(ValueError,'template changed'): self.run_generate()

    def test_snapshot_drift(self):
        self.hw['pci'][0]['device'] = '0xffff'
        with self.assertRaisesRegex(ValueError,'snapshot changed'):
            b.generate(self.hw,self.policy,self.source,self.asl)

    def test_missing_native_ram(self):
        self.hw['ram'] = []
        with self.assertRaisesRegex(ValueError,'RAM template'): self.run_generate()

    def test_sparse_apic(self):
        self.hw['cpus'][7]['apicid'] = 12
        with self.assertRaisesRegex(ValueError,'APIC'): self.run_generate()

    def test_duplicate_bdf(self):
        self.hw['pci'].append(copy.deepcopy(self.hw['pci'][0]))
        with self.assertRaisesRegex(ValueError,'duplicate'): self.run_generate()

    def test_missing_parent(self):
        self.hw['pci'][1]['parent'] = '0000:00:1c.0'
        with self.assertRaisesRegex(ValueError,'parent'): self.run_generate()

    def test_parent_cycle(self):
        self.hw['pci'][0]['parent'] = self.hw['pci'][1]['bdf']
        self.hw['pci'][1]['parent'] = self.hw['pci'][0]['bdf']
        with self.assertRaisesRegex(ValueError,'cycle'): self.run_generate()

    def test_collect_guest_before_reading_hardware(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'proc').mkdir()
            (root/'proc/cmdline').write_text('hvisor.zone0=1')
            (root/'proc/cpuinfo').write_text('vendor_id : GenuineIntel\nflags : vmx')
            with self.assertRaisesRegex(ValueError,'native Linux'): b.collect(root)

    def test_collect_native_fixture(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            files={'proc/cmdline':'root=UUID=test',
                   'proc/cpuinfo':'processor : 0\nvendor_id : GenuineIntel\nphysical id : 0\ncore id : 0\napicid : 0\nflags : vmx',
                   'proc/iomem':'00100000-07ffffff : System RAM'}
            for name in ('board_vendor','board_name','board_version','bios_version'):
                files['sys/class/dmi/id/'+name]='TEST'
            for name,value in files.items():
                dest=root/name; dest.parent.mkdir(parents=True,exist_ok=True); dest.write_text(value)
            pci=root/'sys/bus/pci/devices/0000:00:14.0'; pci.mkdir(parents=True)
            for name,value in {'vendor':'0x8086','device':'0xa2af','class':'0x0c0330',
                               'resource':'00000000df330000 00000000df33ffff 0000000000000200'}.items():
                (pci/name).write_text(value)
            group=root/'sys/kernel/iommu_groups/2'; group.mkdir(parents=True)
            (pci/'iommu_group').symlink_to(group,target_is_directory=True)
            acpi=root/'sys/firmware/acpi/tables'; acpi.mkdir(parents=True)
            for name in ('MCFG','APIC','DMAR','HPET','FACP','DSDT'):
                (acpi/name).write_bytes(bytes.fromhex(table(name,bytearray())))
            hw=b.collect(root)
            self.assertEqual(hw['cpus'][0]['apicid'],0)
            self.assertEqual(hw['pci'][0]['group'],2)
            self.assertEqual(hw['pci'][0]['resources'][0]['end'],0xdf340000)
            self.assertEqual(hw['ram'],[[0x100000,0x8000000]])

    def test_cli_roundtrip(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            snapshot, policy = root/'hardware.json', root/'policy.json'
            snapshot.write_text(b.dump(self.hw))
            common = ['--snapshot',str(snapshot),'--board',str(PROFILE/'board.rs'),
                      '--acpi',str(PROFILE/'minimal-dsdt.asl')]
            command = [sys.executable,str(HERE/'boardgen.py')]
            subprocess.run(command+['policy']+common+['--output',str(policy)],check=True,capture_output=True)
            result = subprocess.run(command+['generate']+common+['--policy',str(policy),'--output',str(root/'rejected')],capture_output=True)
            self.assertEqual(result.returncode,2)
            self.assertFalse((root/'rejected').exists())
            policy.write_text(b.dump(self.policy))
            args = command+['generate']+common+['--policy',str(policy),'--output',str(root/'result')]
            subprocess.run(args,check=True,capture_output=True)
            self.assertEqual(len(list((root/'result').iterdir())),10)
            ET.parse(root/'result/allocation.svg')
            again = subprocess.run(args,capture_output=True)
            self.assertEqual(again.returncode,2)

    @unittest.skipUnless(shutil.which('rustc'), 'rustc not installed')
    def test_generated_rust_syntax_and_interface(self):
        # ABI-shaped stub checks emitted syntax/types, NOT full hvisor semantics.
        prelude = '''
#![allow(dead_code)]
pub mod memory { pub type GuestPhysAddr = u64; }
pub mod pci { pub mod vpci_dev { #[derive(Clone,Copy)] pub enum VpciDevType { Physical } } }
pub mod config {
 pub type BitmapWord = u64;
 pub const MEM_TYPE_RAM: u32 = 0; pub const MEM_TYPE_IO: u32 = 1;
 pub const fn get_irqs_bitmap(a: &[u64;32]) -> [u64;32] { *a }
 pub struct HvConfigMemoryRegion { pub mem_type:u32, pub physical_start:u64, pub virtual_start:u64, pub size:u64 }
 pub struct HvPciConfig { pub bus_range_begin:u64,pub bus_range_end:u64,pub ecam_base:u64,pub ecam_size:u64,
 pub io_base:u64,pub io_size:u64,pub pci_io_base:u64,pub mem32_base:u64,pub mem32_size:u64,pub pci_mem32_base:u64,
 pub mem64_base:u64,pub mem64_size:u64,pub pci_mem64_base:u64,pub domain:u64 }
 pub struct HvPciDevConfig { pub domain:u8,pub bus:u8,pub device:u8,pub function:u8,
 pub v_bus:u8,pub v_device:u8,pub v_function:u8,pub dev_type:crate::pci::vpci_dev::VpciDevType }
}
pub mod arch { pub mod zone { pub struct HvArchZoneConfig {
 pub ioapic_base:usize,pub ioapic_size:usize,pub kernel_entry_gpa:u64,pub cmdline_load_gpa:u64,
 pub setup_load_gpa:u64,pub initrd_load_gpa:u64,pub initrd_size:usize,pub rsdp_memory_region_id:usize,
 pub acpi_memory_region_id:usize,pub uefi_memory_region_id:usize,pub screen_base:u64
} } }
#[macro_export] macro_rules! pci_dev {
 ($d:expr,$b:expr,$s:expr,$f:expr => $vb:expr,$vs:expr,$vf:expr,$t:expr) => {
 crate::config::HvPciDevConfig {domain:$d,bus:$b,device:$s,function:$f,v_bus:$vb,v_device:$vs,v_function:$vf,dev_type:$t}
 }; }
'''
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)
            (path/'board.rs').write_text(self.run_generate())
            (path/'test.rs').write_text(prelude+'\nmod board;\n')
            result=subprocess.run(['rustc','--crate-type=lib',str(path/'test.rs'),'-o',str(path/'test.rlib')],capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stderr)

    def test_hypervisor_overlap(self):
        self.policy['hypervisor_reserved'] = [0x200000,0x5100000]
        with self.assertRaisesRegex(ValueError,'exposes hypervisor'): self.run_generate()

    def test_bad_acpi_checksum(self):
        self.hw['acpi']['MCFG'] = self.hw['acpi']['MCFG'][:-2]+'01'
        with self.assertRaisesRegex(ValueError,'checksum'): self.run_generate()

    def test_rmrr_not_silently_ignored(self):
        self.hw['acpi']['DMAR'] = table('DMAR',bytearray(12)+struct.pack('<HH',1,4))
        with self.assertRaisesRegex(ValueError,'RMRR'): self.run_generate()

    def test_new_bar_requires_review(self):
        self.hw['pci'][1]['resources'] = [dict(index=0,start=0xdd000000,end=0xdd001000,flags=0x200)]
        with self.assertRaisesRegex(ValueError,'approval'): self.run_generate()
        self.policy['approved_extra_bar_pages'] = [[0xdd000000,0xdd001000]]
        result = self.run_generate()
        self.assertIn('ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; 31]', result)
        self.assertIn('physical_start: 0xdd000000', result)

    def test_bar_cannot_expose_vtd(self):
        self.hw['pci'][1]['resources'] = [dict(index=0,start=0xfed90000,end=0xfed91000,flags=0x200)]
        with self.assertRaisesRegex(ValueError,'protected resource'): self.run_generate()

    def test_output_never_overwrites(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'test'
            b.save_new(path,'original')
            with self.assertRaises(FileExistsError): b.save_new(path,'changed')
            self.assertEqual(path.read_text(),'original')


if __name__ == '__main__':
    unittest.main()
