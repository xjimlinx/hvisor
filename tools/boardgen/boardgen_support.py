"""Portable reports and generated package assets. No remote or deployment writes."""
import hashlib
import json
import re
import struct


def capabilities(hw):
    checks=[]
    def add(name,status,detail):
        checks.append(dict(name=name,status=status,detail=detail))
    add('native-x86', 'pass' if hw.get('native_capture') is True and hw.get('machine')=='x86_64' else 'fail',
        'Snapshot provenance assertion; not independently attested.')
    flags=hw.get('cpu_flags')
    add('vmx', 'unknown' if flags is None else ('pass' if 'vmx' in flags else 'fail'), 'Linux CPU feature flags')
    ids=sorted(c.get('apicid',-1) for c in hw.get('cpus',[]))
    add('dense-apic-8max','pass' if 0<len(ids)<=8 and ids==list(range(len(ids))) else 'fail',str(ids))
    add('iommu-groups','pass' if hw.get('pci') and all(d.get('group') is not None for d in hw['pci']) else 'fail',
        'Native groups are evidence of Linux grouping, not hvisor isolation.')
    for name in ('MCFG','APIC','DMAR','HPET','FACP','DSDT'):
        raw=bytes.fromhex(hw.get('acpi',{}).get(name,''))
        valid=len(raw)>=36 and raw[:4]==name.encode() and struct.unpack_from('<I',raw,4)[0]==len(raw) and sum(raw)%256==0
        add('acpi-'+name,'pass' if valid else 'fail','Header/length/checksum only; AML semantics not validated.')
    for feature in ('vmx-control-msrs','ept-capabilities','vtd-interrupt-remapping','reset-isolation','firmware-address-stability'):
        add(feature,'unknown','Not proven by this read-only sysfs snapshot; platform adapter / hardware test required.')
    return dict(status='blocked' if any(c['status']=='fail' for c in checks) else 'needs-review',checks=checks,
                boot_ready=False)


def migration_diff(before,after):
    changes=[]
    def add(kind,key,old,new):
        if old!=new: changes.append(dict(kind=kind,key=key,before=old,after=new))
    for key in ('machine','dmi','cpus','cpu_flags','ram'):
        add('platform',key,before.get(key),after.get(key))
    old={d['bdf']:d for d in before['pci']}; new={d['bdf']:d for d in after['pci']}
    for bdf in sorted(set(old)|set(new)):
        if bdf not in old: add('device-added',bdf,None,new[bdf])
        elif bdf not in new: add('device-removed',bdf,old[bdf],None)
        else:
            for key in ('vendor','device','pci_class','parent','group','resources','driver'):
                add('device-'+key,bdf,old[bdf].get(key),new[bdf].get(key))
    for name in sorted(set(before.get('acpi',{}))|set(after.get('acpi',{}))):
        digest=lambda s: hashlib.sha256(bytes.fromhex(s)).hexdigest() if s else None
        add('acpi',name,digest(before.get('acpi',{}).get(name)),digest(after.get('acpi',{}).get(name)))
    return dict(changes=changes,regeneration_required=bool(changes),automatic_identity_matching=False,
                note='BDF is a location, not a persistent device identity. No changes does not prove boot compatibility.')


def subtract(ranges,exclusions):
    result=[]
    for start,end in sorted(ranges):
        parts=[(start,end)]
        for lo,hi in exclusions:
            parts=[piece for a,b in parts for piece in ((a,min(b,lo)),(max(a,hi),b)) if piece[0]<piece[1]]
        result.extend(parts)
    return [list(p) for p in result]


def layout_report(hw,policy,board,parse_regions,constant):
    regions=parse_regions(board)
    ram=[r for r in regions if r['kind']=='MEM_TYPE_RAM']
    exclusions=[policy['hypervisor_reserved']]+[[r['physical_start'],r['physical_start']+r['size']] for r in regions]
    unused=subtract(hw['ram'],exclusions)
    # Page-align proposals; never infer RAM from unlisted holes.
    proposals=[[((lo+4095)//4096)*4096,(hi//4096)*4096] for lo,hi in unused
               if ((lo+4095)//4096)*4096 < (hi//4096)*4096]
    boot={}
    for key in ('ROOT_ZONE_KERNEL_ADDR','ROOT_ZONE_INITRD_ADDR','ROOT_ZONE_INITRD_SIZE',
                'ROOT_ZONE_CMDLINE_ADDR','ROOT_ZONE_SETUP_ADDR','ROOT_ZONE_SCREEN_BASE_ADDR'):
        boot[key]=constant(board,key)
    return dict(mode='audited-layout-plus-unassigned-RAM-proposals',regions=regions,
        guest_ram_bytes=sum(r['size'] for r in ram),hypervisor_reserved=policy['hypervisor_reserved'],
        boot_constants=boot,unassigned_native_ram=proposals,automatic_ram_relocation=False,
        warnings=['Proposed ranges are not inserted into board.rs.',
                  'Kernel size, decompression workspace, framebuffer and load-time overlap need image/ELF checks.',
                  'Special ACPI/UEFI RAM mappings are included in guest_ram_bytes; Linux MemTotal will differ.'])


def platform_files(template,name,board,asl):
    if not re.fullmatch(r'[a-z][a-z0-9_-]{0,39}',name):
        raise ValueError('invalid generated platform name')
    required=['linker.ld','platform.mk','cargo/config.template.toml','kconfig/defconfig']
    content={f: (template/f).read_text() for f in required}
    prefix=f'platform/x86_64/{name}/'
    files={prefix+f:text for f,text in content.items()}
    files[prefix+'board.rs']=board
    files[prefix+'minimal-dsdt.asl']=asl
    files[prefix+'boardgen-profile']='intel-trusted-zone0-v1\n'
    files['BUILD.md']=f'''# Generated platform candidate

Copy the `platform/x86_64/{name}` directory into a separate worktree of the
matching hvisor revision. The build.rs boardgen-profile support is required.
Then run: `make ARCH=x86_64 BOARD={name} MODE=release LOG=info elf`.

This copies reviewed linker/Kconfig/Cargo templates, not a universal chipset
adapter. Check CPU_NUM, CPU feature contracts, font/include dependencies and
the generated report first. Guest kernel/initrd/firmware blobs are NOT included.
No GRUB, firmware, filesystem or remote system was changed by generation.
After compilation check the actual ELF reserved range against policy.json.
Do not replace the known-working boot entry. No automatic reboot is provided.
'''
    files['template-manifest.json']=json.dumps({k:hashlib.sha256(v.encode()).hexdigest() for k,v in content.items()},indent=2)+'\n'
    return files


def acceptance_script(hw):
    expected={d['bdf']:dict(vendor=d['vendor'],device=d['device']) for d in hw['pci']}
    return '''#!/usr/bin/env python3
"""Run inside the generated identity-BDF Zone0. Read-only observations only.
Usage: python3 acceptance.py > acceptance.json
No stress tests, device resets, network connections or service changes.
"""
import json
import subprocess
from pathlib import Path
EXPECTED = '''+repr(expected)+'''
CPU_COUNT = '''+repr(len(hw['cpus']))+'''
def text(path):
    try: return Path(path).read_text().strip()
    except OSError: return None
def command(args):
    try:
        p=subprocess.run(args,capture_output=True,text=True,timeout=15)
        return dict(returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
    except (OSError,subprocess.TimeoutExpired) as e: return dict(error=str(e))
cmdline=text('/proc/cmdline') or ''
devices=[]
for bdf,identity in EXPECTED.items():
    p=Path('/sys/bus/pci/devices')/bdf
    actual={key:text(p/key) for key in identity}
    devices.append(dict(bdf=bdf,identity_ok=actual==identity,actual=actual,
        driver=(p/'driver').resolve().name if (p/'driver').exists() else None,
        msi_vectors=sorted(x.name for x in (p/'msi_irqs').glob('*'))))
report=dict(hvisor_zone0='hvisor.zone0=1' in cmdline.split(),expected_cpu_count=CPU_COUNT,
    online_cpus=text('/sys/devices/system/cpu/online'),meminfo=text('/proc/meminfo'),
    boot_id=text('/proc/sys/kernel/random/boot_id'),devices=devices,
    failed_units=command(['systemctl','--failed','--no-pager']),
    mounts=command(['findmnt','-J']),addresses=command(['ip','-j','address']),
    kernel_warnings=command(['journalctl','-b','-k','-p','warning','--no-pager']),
    clocksource=text('/sys/devices/system/clocksource/clocksource0/current_clocksource'),
    status='observations-only-not-functional-certification',
    manual_tests=['USB transfer','audio playback/capture','GPU workload','management reconnect','storage integrity'])
online=[]
for part in (report['online_cpus'] or '').split(','):
    if part:
        ends=[int(n) for n in part.split('-')]
        online.extend(range(ends[0],ends[-1]+1))
report['checks']=dict(zone0_marker=report['hvisor_zone0'],
    cpu_ids=online==list(range(CPU_COUNT)),pci_identity=all(d['identity_ok'] for d in devices))
report['status']='basic-checks-pass-functional-tests-pending' if all(report['checks'].values()) else 'basic-checks-failed'
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(0 if all(report['checks'].values()) else 1)
'''
