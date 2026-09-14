#!/usr/bin/env python3
"""Offline, fail-closed x86 board candidate generator. Python 3.11+, stdlib only."""
import argparse
import hashlib
import json
import platform
import re
import struct
from pathlib import Path
from xml.sax.saxutils import escape
from boardgen_support import capabilities, migration_diff, layout_report, platform_files, acceptance_script


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return path.read_text().strip()


def save_new(path, data):
    with path.open('x') as f:
        f.write(data)


def dump(data):
    return json.dumps(data, indent=2, ensure_ascii=False) + '\n'


def integer(s):
    return int(s.replace('_', ''), 0)


def check(condition, message):
    if not condition:
        raise ValueError(message)


def acpi_table(data, signature):
    check(len(data) >= 36 and data[:4] == signature.encode(), 'invalid ACPI signature')
    check(struct.unpack_from('<I', data, 4)[0] == len(data), 'invalid ACPI length')
    check(sum(data) % 256 == 0, 'invalid ACPI checksum')
    return data


def collect(root):
    """No PCI config/BAR reads, driver unbinding, bus probes or MSR writes."""
    cmdline = read(root / 'proc/cmdline')
    cpu = read(root / 'proc/cpuinfo')
    check('hvisor.zone0=' not in cmdline, 'collect from native Linux, not hvisor Zone0')
    check(not re.search(r'\bhypervisor\b', cpu), 'virtualized CPU detected; native capture required')
    check('GenuineIntel' in cpu and re.search(r'\bvmx\b', cpu), 'Intel VMX required')
    cpus = []
    for block in cpu.split('\n\n'):
        fields = dict(re.findall(r'^([^:\n]+)\s*:\s*(.*)$', block, re.M))
        fields = {k.strip(): v for k, v in fields.items()}
        if 'processor' in fields:
            cpus.append({k: int(fields[k]) for k in ('processor', 'physical id', 'core id', 'apicid')})
    ram = []
    for lo, hi in re.findall(r'^\s*([0-9a-f]+)-([0-9a-f]+) : System RAM$', read(root / 'proc/iomem'), re.M):
        start, end = int(lo, 16), int(hi, 16) + 1
        check(end > 1, 'iomem addresses are redacted; run collector as root')
        ram.append([start, end])
    check(ram, 'no System RAM ranges found')
    devices = []
    for path in sorted((root / 'sys/bus/pci/devices').iterdir()):
        if not re.fullmatch(r'0000:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]', path.name):
            raise ValueError('only PCI segment 0000 supported')
        group = path / 'iommu_group'
        check(group.exists(), f'{path.name}: missing IOMMU group; enable native IOMMU')
        resources = []
        # sysfs resource file is kernel-maintained metadata; never mmap resourceN.
        for index, line in enumerate(read(path / 'resource').splitlines()):
            start, end, flags = (int(v, 16) for v in line.split())
            if flags and (start or end):
                resources.append(dict(index=index, start=start, end=end+1, flags=flags))
        ancestors = [p.name for p in path.resolve().parents
                     if re.fullmatch(r'0000:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]', p.name)]
        devices.append(dict(bdf=path.name, vendor=read(path/'vendor'), device=read(path/'device'),
                            pci_class=read(path/'class'), group=int(group.resolve().name),
                            parent=ancestors[0] if ancestors else None, resources=resources,
                            driver=(path/'driver').resolve().name if (path/'driver').exists() else None))
    tables = {}
    for name in ('MCFG', 'APIC', 'DMAR', 'HPET', 'FACP', 'DSDT'):
        data = acpi_table((root/'sys/firmware/acpi/tables'/name).read_bytes(), name)
        tables[name] = data.hex()
    dmi = {name: read(root/'sys/class/dmi/id'/name)
           for name in ('board_vendor', 'board_name', 'board_version', 'bios_version')}
    return dict(schema=1, native_capture=True, machine=platform.machine(), dmi=dmi,
                cpus=cpus, ram=ram, pci=devices, acpi=tables,
                cpu_flags=sorted(set(re.findall(r'^flags\s*:\s*(.*)$', cpu, re.M)[0].split())),
                notes=['No serial numbers, credentials or network profiles collected.',
                       'Capture is not proof that firmware resources remain unchanged after reboot.'])


def memory_regions(source):
    source = re.sub(r'//[^\n]*|/\*.*?\*/', '', source, flags=re.S)
    result = []
    for body in re.findall(r'HvConfigMemoryRegion\s*\{([^}]+)\}', source):
        kind = re.search(r'mem_type:\s*(MEM_TYPE_\w+)', body)
        check(kind is not None, 'template has unsupported memory region syntax')
        row = {k: integer(re.search(k+r':\s*(0x[\da-fA-F_]+|\d+)', body)[1])
               for k in ('physical_start', 'virtual_start', 'size')}
        row['kind'] = kind[1]
        result.append(row)
    check(result, 'template has no memory regions')
    return result


def overlaps(a, b):
    return a[0] < b[1] and b[0] < a[1]


def covered(start, end, ranges):
    for lo, hi in sorted(ranges):
        if lo <= start < hi:
            start = max(start, hi)
        if start >= end:
            return True
    return False


def constant(source, name):
    return integer(re.search(r'\b'+name+r':[^=]+?=\s*(0x[\da-fA-F_]+|\d+)', source)[1])


def generate(hw, policy, source, asl):
    check(hw['schema'] == 1 and hw['native_capture'] is True, 'native schema 1 snapshot required')
    check(hw['machine'] == 'x86_64', 'only x86_64 supported')
    check(policy['profile'] == 'intel-trusted-zone0-v1', 'unsupported profile')
    check(policy['reviewed'] is True, 'policy is unreviewed; review boot layout/ACPI/quirks first')
    check(policy['board_sha256'] == digest(source.encode()), 'board template changed')
    check(policy['acpi_sha256'] == digest(asl.encode()), 'ACPI template changed')
    check(policy['dmi'] == hw['dmi'], 'DMI/BIOS mismatch; re-review policy')
    check(policy['snapshot_sha256'] == digest(dump(hw).encode()), 'snapshot changed; re-review policy')
    cpus = hw['cpus']
    # Current x86 BSP/AP setup is not a generic sparse-APIC topology builder.
    check([c['processor'] for c in cpus] == list(range(len(cpus))), 'sparse/offline CPUs unsupported')
    check(sorted(c['apicid'] for c in cpus) == list(range(len(cpus))), 'sparse APIC IDs unsupported')
    check(0 < len(cpus) <= 8, 'current profile supports at most eight hardware threads')
    check(constant(source, 'BOARD_NCPUS') == len(cpus), 'CPU/ACPI template count mismatch')
    check(constant(source, 'ROOT_ZONE_CPUS') == (1 << len(cpus))-1, 'template must assign all CPUs')
    mcfg = acpi_table(bytes.fromhex(hw['acpi']['MCFG']), 'MCFG')
    check(len(mcfg) == 60, 'exactly one MCFG segment required')
    ecam, segment, first, last = struct.unpack_from('<QHBB', mcfg, 44)
    check(segment == first == 0, 'segment and first bus must be zero')
    check(ecam == integer(re.search(r'ecam_base:\s*(0x[\da-fA-F_]+)', source)[1]), 'ECAM base mismatch')
    maxbus = constant(source, 'ROOT_PCI_MAX_BUS')
    check(maxbus <= last, 'template exceeds physical MCFG bus range')
    pci = hw['pci']
    bdfs = [d['bdf'] for d in pci]
    check(0 < len(bdfs) <= 32 and len(bdfs) == len(set(bdfs)), 'duplicate/too many PCI functions')
    for d in pci:
        check(re.fullmatch(r'0000:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]', d['bdf']), 'invalid BDF')
        check(d['parent'] is None or d['parent'] in bdfs, 'missing PCI parent')
        check(d['group'] is not None, 'missing native IOMMU group')
        check(int(d['bdf'][5:7],16) <= maxbus, 'ACPI/board bus range must be reviewed')
    parents = {d['bdf']: d['parent'] for d in pci}
    for node in parents:
        seen = set()
        while node is not None:
            check(node not in seen, 'cycle in PCI parent topology')
            seen.add(node)
            node = parents[node]
    regs = memory_regions(source)
    reserved = policy['hypervisor_reserved']
    check(len(reserved) == 2 and 0 <= reserved[0] < reserved[1], 'invalid hvisor reservation')
    protected = [reserved, [0xfee00000,0xfee01000], [0xfec00000,0xfec01000],
                 [ecam,ecam+(last+1)*0x100000]]
    dmar = acpi_table(bytes.fromhex(hw['acpi']['DMAR']), 'DMAR')
    pos = 48
    while pos < len(dmar):
        kind, length = struct.unpack_from('<HH', dmar, pos)
        check(length >= 4 and pos+length <= len(dmar), 'invalid DMAR structure')
        if kind == 0:
            check(length >= 16, 'short DRHD')
            addr = struct.unpack_from('<Q', dmar, pos+8)[0]
            protected.append([addr,addr+0x1000])
        elif kind == 1:
            raise ValueError('DMAR RMRR needs an explicit platform adapter; refusing generic output')
        pos += length
    for r in regs:
        span = [r['physical_start'],r['physical_start']+r['size']]
        check(r['size'] > 0 and all(v % 4096 == 0 for v in (r['physical_start'],r['virtual_start'],r['size'])), 'unaligned template region')
        check(not any(overlaps(span,p) for p in protected), 'template exposes hypervisor/APIC/VT-d/ECAM')
        if r['kind'] == 'MEM_TYPE_RAM':
            check(covered(*span, hw['ram']), f'RAM template not backed by native RAM: {span}')
        else:
            check(r['kind'] == 'MEM_TYPE_IO', 'unsupported memory type')
            check(not any(overlaps(span,p) for p in hw['ram']), 'IO overlaps native RAM')
    added = []
    for d in pci:
        for bar in d['resources']:
            # Only endpoint BAR0..5; ROM/bridge windows are NOT guest mappings.
            if bar['index'] > 5 or integer(d['pci_class']) >> 8 == 0x0604 or not bar['flags'] & 0x200:
                continue
            lo, hi = bar['start'] & ~4095, (bar['end']+4095)&~4095
            check(lo > 0 and hi > lo, 'unassigned/invalid BAR')
            check(not any(overlaps([lo,hi],p) for p in protected+hw['ram']), 'BAR hits protected resource')
            io = [[r['physical_start'],r['physical_start']+r['size']] for r in regs if r['kind']=='MEM_TYPE_IO' and r['physical_start']==r['virtual_start']]
            if not covered(lo,hi,io):
                check(not any(overlaps([lo,hi],p) for p in io), 'partial/shared-page BAR overlap needs review')
                check([lo,hi] in policy['approved_extra_bar_pages'], f'new BAR pages need approval: {[lo,hi]}')
                reg = dict(kind='MEM_TYPE_IO', physical_start=lo, virtual_start=lo, size=hi-lo)
                regs.append(reg)
                added.append(reg)
    for i,r in enumerate(regs):
        for other in regs[i+1:]:
            for key in ('physical_start','virtual_start'):
                check(not overlaps([r[key],r[key]+r['size']],[other[key],other[key]+other['size']]), 'memory overlap')
    check(len(regs) <= 64, 'memory region ABI limit exceeded')
    # Retain reviewed boot/ACPI region indices; only append explicitly approved BARs.
    match = re.search(r'pub const ROOT_ZONE_MEMORY_REGIONS: \[HvConfigMemoryRegion; (\d+)\] = \[(.*?)\n\];', source, re.S)
    check(match is not None, 'unsupported board memory array')
    extra = ''.join('\n    HvConfigMemoryRegion { mem_type: MEM_TYPE_IO, physical_start: %#x, virtual_start: %#x, size: %#x },' % (r['physical_start'],r['virtual_start'],r['size']) for r in added)
    source = source[:match.start()] + f'pub const ROOT_ZONE_MEMORY_REGIONS: [HvConfigMemoryRegion; {int(match[1])+len(added)}] = ['+match[2]+extra+'\n];'+source[match.end():]
    # Z270 inventory block is the supported template interface in v1.
    match = re.search(r'macro_rules! zone0_native_inventory.*?\nzone0_native_inventory!\s*\{.*?\n\}', source, re.S)
    check(match is not None, 'template must expose zone0_native_inventory block')
    rows = []
    for bdf in sorted(bdfs):
        bus, dev, fun = int(bdf[5:7],16), int(bdf[8:10],16), int(bdf[11],16)
        rows.append(f'    pci_dev!(0, {bus}, 0x{dev:02x}, {fun} => {bus}, 0x{dev:02x}, {fun}, VpciDevType::Physical),')
    source = source[:match.start()] + f'pub const ROOT_PCI_DEVS: [HvPciDevConfig; {len(rows)}] = [\n'+'\n'.join(rows)+'\n];'+source[match.end():]
    return '// GENERATED CANDIDATE: review report.json before use. Not hardware-validated.\n'+source


def diagram(hw):
    height = 120 + 38*len(hw['pci'])
    lines = [f'<svg xmlns="http://www.w3.org/2000/svg" width="1050" height="{height}" viewBox="0 0 1050 {height}">',
             '<title>Zone0 generated PCI inventory — candidate only</title>',
             '<rect width="100%" height="100%" fill="#f4f7fa"/>',
             '<g font-family="Noto Sans CJK SC, sans-serif" font-size="16" fill="#243b53">',
             '<text x="30" y="40">Zone0 · 生成候选 / 物理 BDF、上游与 IOMMU 组</text>']
    for i,d in enumerate(hw['pci']):
        label = f"{d['parent'] or 'root'} → {d['bdf']}    G{d['group']}    {d['vendor']}:{d['device']}    {d['driver'] or 'no driver'}"
        lines.append(f'<text x="30" y="{85+i*38}">{escape(label)}</text>')
    return '\n'.join(lines)+ '\n</g></svg>\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    c = sub.add_parser('collect'); c.add_argument('--output', type=Path, required=True)
    p = sub.add_parser('policy'); p.add_argument('--snapshot', type=Path, required=True)
    p.add_argument('--board', type=Path, required=True); p.add_argument('--acpi', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    g = sub.add_parser('generate')
    for name in ('snapshot','policy','board','acpi','output'):
        g.add_argument('--'+name, type=Path, required=True)
    g.add_argument('--platform-template', type=Path)
    g.add_argument('--name', default='generated-intel')
    c = sub.add_parser('check')
    c.add_argument('--snapshot',type=Path,required=True)
    c.add_argument('--output',type=Path,required=True)
    d = sub.add_parser('diff')
    d.add_argument('--before',type=Path,required=True)
    d.add_argument('--after',type=Path,required=True)
    d.add_argument('--output',type=Path,required=True)
    a = parser.parse_args()
    try:
        if a.command == 'collect':
            save_new(a.output, dump(collect(Path('/')))); return
        if a.command == 'check':
            save_new(a.output,dump(capabilities(json.loads(a.snapshot.read_text())))); return
        if a.command == 'diff':
            save_new(a.output,dump(migration_diff(json.loads(a.before.read_text()),json.loads(a.after.read_text())))); return
        hw = json.loads(a.snapshot.read_text())
        source, asl = a.board.read_text(), a.acpi.read_text()
        if a.command == 'policy':
            save_new(a.output, dump(dict(profile='intel-trusted-zone0-v1', reviewed=False,
                snapshot_sha256=digest(dump(hw).encode()), dmi=hw['dmi'],
                board_sha256=digest(source.encode()), acpi_sha256=digest(asl.encode()),
                hypervisor_reserved=[0,0], approved_extra_bar_pages=[],
                review=['Set reserved range from actual hvisor ELF; audit RAM/boot addresses.',
                        'Audit native ACPI, IRQ routing, bridge/reset and device quirks.',
                        'Confirm root UUID, initrd size, framebuffer and CPU topology.',
                        'Review all existing IO mappings and any extra BAR pages.']))); return
        policy = json.loads(a.policy.read_text())
        board = generate(hw, policy, source, asl)
        extras = platform_files(a.platform_template,a.name,board,asl) if a.platform_template else {}
        # Never overwrite files or install into a board directory implicitly.
        a.output.mkdir(parents=False, exist_ok=False)
        save_new(a.output/'board.rs', board)
        save_new(a.output/'minimal-dsdt.asl', asl)
        save_new(a.output/'hardware.json', dump(hw))
        save_new(a.output/'policy.json', dump(policy))
        save_new(a.output/'allocation.svg', diagram(hw))
        save_new(a.output/'capabilities.json',dump(capabilities(hw)))
        save_new(a.output/'memory-layout.json',dump(layout_report(hw,policy,board,memory_regions,constant)))
        save_new(a.output/'acceptance.py',acceptance_script(hw))
        for relative, content in extras.items():
            destination=a.output/relative
            destination.parent.mkdir(parents=True,exist_ok=True)
            save_new(destination,content)
        save_new(a.output/'report.json', dump(dict(status='candidate-not-boot-validated',
            pci_count=len(hw['pci']), board_sha256=digest(board.encode()),
            automated=['snapshot/template identity','RAM containment','BAR coverage','protected ranges','unique PCI BDFs'],
            manual=['ACPI AML/INTx correctness','boot image placement and sizes','device reset/DMA handover',
                    'compile + final ELF overlap check','actual reboot and functional tests'],
            acpi_mode='reviewed template copied unchanged; no automatic AML translation',
            deployment='none')))
        save_new(a.output/'manifest.json',dump({str(p.relative_to(a.output)):digest(p.read_bytes())
                  for p in sorted(a.output.rglob('*')) if p.is_file()}))
        print(a.output)
    except (ValueError, KeyError, OSError, struct.error, TypeError, AttributeError, IndexError) as exc:
        parser.exit(2, f'boardgen: {exc}\n')


if __name__ == '__main__':
    main()
