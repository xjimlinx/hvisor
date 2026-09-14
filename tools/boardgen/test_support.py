import ast
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from test_boardgen import fixture, b, PROFILE, HERE
from boardgen_support import capabilities, migration_diff, subtract, layout_report, platform_files, acceptance_script


class SupportTests(unittest.TestCase):
    def test_unknown_capabilities_are_not_pass(self):
        hw,_,_,_=fixture()
        result=capabilities(hw)
        self.assertFalse(result['boot_ready'])
        self.assertEqual(next(c['status'] for c in result['checks'] if c['name']=='vmx'),'unknown')
        hw['cpu_flags']=[]
        self.assertEqual(capabilities(hw)['status'],'blocked')

    def test_diff_resources_and_driver(self):
        old,_,_,_=fixture()
        new=json.loads(json.dumps(old))
        self.assertFalse(migration_diff(old,new)['regeneration_required'])
        new['pci'][1]['resources'][0]['start']+=4096
        new['pci'][1]['driver']='other'
        types={c['kind'] for c in migration_diff(old,new)['changes']}
        self.assertEqual(types,{'device-resources','device-driver'})

    def test_subtract(self):
        self.assertEqual(subtract([[0,100]],[[20,40],[30,60],[80,120]]),[[0,20],[60,80]])
        self.assertEqual(subtract([[100,200]],[[0,50],[250,300]]),[[100,200]])

    def test_layout(self):
        hw,p,source,_=fixture()
        result=layout_report(hw,p,source,b.memory_regions,b.constant)
        self.assertEqual(result['unassigned_native_ram'],[])
        self.assertFalse(result['automatic_ram_relocation'])
        hw['ram'].append([0x900000000,0x900010000])
        self.assertEqual(layout_report(hw,p,source,b.memory_regions,b.constant)['unassigned_native_ram'],[[0x900000000,0x900010000]])

    def test_acceptance_is_valid_python(self):
        hw,_,_,_=fixture()
        ast.parse(acceptance_script(hw))

    def test_package_and_path_guard(self):
        _,_,board,asl=fixture()
        files=platform_files(PROFILE,'test-intel',board,asl)
        self.assertIn('platform/x86_64/test-intel/linker.ld',files)
        self.assertEqual(files['platform/x86_64/test-intel/boardgen-profile'],'intel-trusted-zone0-v1\n')
        with self.assertRaises(ValueError): platform_files(PROFILE,'../../escape',board,asl)

    def test_package_cli_manifest(self):
        hw,p,_,_=fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'hw.json').write_text(b.dump(hw)); (root/'p.json').write_text(b.dump(p))
            subprocess.run([sys.executable,str(HERE/'boardgen.py'),'generate',
                '--snapshot',str(root/'hw.json'),'--policy',str(root/'p.json'),
                '--board',str(PROFILE/'board.rs'),'--acpi',str(PROFILE/'minimal-dsdt.asl'),
                '--platform-template',str(PROFILE),'--name','test-intel','--output',str(root/'out')],check=True,capture_output=True)
            manifest=json.loads((root/'out/manifest.json').read_text())
            for name,sha in manifest.items():
                self.assertEqual(b.digest((root/'out'/name).read_bytes()),sha)
            for command,flags in [('check',['--snapshot',str(root/'hw.json')]),
                                  ('diff',['--before',str(root/'hw.json'),'--after',str(root/'hw.json')])]:
                subprocess.run([sys.executable,str(HERE/'boardgen.py'),command,*flags,
                    '--output',str(root/(command+'.json'))],check=True,capture_output=True)


if __name__=='__main__': unittest.main()
