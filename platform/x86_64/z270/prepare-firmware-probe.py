#!/usr/bin/env python3
"""Build a RAM-only reset ROM and review manifest; never deploy or open /dev/hvisor."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    obj, elf, rom = args.output/'probe.o', args.output/'probe.elf', args.output/'probe.rom'
    subprocess.run(['as','--32',str(Path(__file__).with_name('firmware-probe.S')),
                    '-o',str(obj)], check=True)
    subprocess.run(['ld','-m','elf_i386','-Ttext=0',
                    '-o',str(elf),str(obj)], check=True)
    # Some assemblers emit an allocated GNU property note. Only .text is ROM.
    subprocess.run(['objcopy','-O','binary','-j','.text',str(elf),str(rom)], check=True)
    data = rom.read_bytes()
    assert len(data) == 65536
    assert data[0xfff0] == 0xe9
    displacement = int.from_bytes(data[0xfff1:0xfff3], 'little', signed=True)
    assert (0xfff3 + displacement) & 0xffff == 0
    manifest = dict(stage='reset-probe-only', deployable=False,
        reason='capability-negotiated loader and real-machine review still required',
        zone_id=1, cpus=[2,3,6,7], boot_mode=2, reset_vector='0xfffffff0',
        expected_serial='HVFW0', pci_devices=[], block_devices=[],
        rom_sha256=hashlib.sha256(data).hexdigest(),
        ram=[dict(hpa='0x5f0000000',gpa='0x0',size='0x1000000'),
             dict(hpa='0x5f1000000',gpa='0xffff0000',size='0x10000')],
        warning='NOT UEFI/OVMF; never load at physical address 0xffff0000; '
                'do not stop the running disk-backed Zone1 to run this plan')
    (args.output/'probe-plan.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2))

if __name__ == '__main__':
    main()
