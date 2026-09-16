#!/usr/bin/env python3
"""Z270 split-USB candidate contract; not a hardware or isolation test."""
import re
from pathlib import Path
root = Path(__file__).resolve().parent
board = (root / "board.rs").read_text()
asl = (root / "minimal-dsdt.asl").read_text()
def number(value):
    return int(value.replace("_", ""), 0)
regions = []
for block in re.findall(r"HvConfigMemoryRegion\s*\{([^}]+)\}", board):
    regions.append({key: number(re.search(key + r"\s*:\s*(0x[\da-fA-F_]+|[0-9]+)", block)[1])
                    for key in ("physical_start", "virtual_start", "size")})
for i, region in enumerate(regions):
    for other in regions[i+1:]:
        for key in ("physical_start", "virtual_start"):
            assert not (region[key] < other[key] + other["size"] and
                        other[key] < region[key] + region["size"]), "region overlap"
for protected in (0xfee00000, 0xfec00000, 0xfed90000):
    assert not any(r["physical_start"] <= protected < r["physical_start"] + r["size"] for r in regions)
bars = [(0xde000000,0x1000000),(0xc0000000,0x10000000),(0xd0000000,0x2000000),
        (0xdf080000,0x4000),(0xdf100000,0x4000),
        (0xdf300000,0x20000),(0xdf330000,0x10000),(0xdf340000,0x4000),
        (0xdf320000,0x10000),(0xdf344000,0x4000),(0xdf348000,0x2000),
        (0xdf34a000,0x1000),(0xdf34b000,0x1000),(0xdf34c000,0x1000),
        (0xdf34d000,0x1000)]
for start, size in bars + [(0xfe000000,0x11000)]:
    assert any(r == dict(physical_start=start,virtual_start=start,size=size) for r in regions)
for start, size in bars:
    assert any(low <= start and start + size <= high for low, high in
               [(0xde000000,0xdf400000),(0xc0000000,0xd2000000)])
rows = re.findall(r'\("([\w-]+)",\s*(\d+),\s*(0x[0-9a-f]+),\s*(\d+),\s*(\d+),\s*"([\w-]+)"\)', board)
inventory = {name: (number(bus), number(dev), number(fun), int(group), parent)
             for name, bus, dev, fun, group, parent in rows}
assert len(rows) == len(inventory) == 17
bdfs = {value[:3] for value in inventory.values()}
assert len(bdfs) == 17
assert bdfs == {(0,d,f) for d,f in [(0,0),(1,0),(0x14,0),(0x16,0),(0x17,0),
 (0x1b,0),(0x1c,0),(0x1c,7),(0x1d,0),
 (0x1f,0),(0x1f,2),(0x1f,3),(0x1f,4),(0x1f,6)]} | {(1,0,0),(1,0,1),(5,0,0)}
for name, (bus, dev, fun, group, parent) in inventory.items():
    assert parent == "root" or parent in inventory
    assert 0 <= bus <= 6 and 0 <= dev < 32 and 0 <= fun < 8
assert inventory["gpu"][3:] == inventory["hdmi"][3:] == (1, "peg")
assert inventory["wifi"][-1] == "rp8"
assert (0,0x1c,4) not in bdfs and (4,0,0) not in bdfs
assert not any(r['physical_start'] < 0xdf208000 and
               r['physical_start'] + r['size'] > 0xdf200000 for r in regions)
assert 'Z270_RESERVED_USB_PATH' in board
assert {inventory[name][3] for name in ("lpc", "pmc", "pch-hda", "smbus")} == {10}
assert '$bus, $dev, $fun => $bus, $dev, $fun' in board
assert 'Z270_PCI_INVENTORY.len()]' in board
assert "pub const ROOT_PCI_MAX_BUS: usize = 6;" in board
assert "ecam_size: 0x700_000" in board
assert "0xDE000000, 0xDF3FFFFF, 0, 0x1400000" in asl
assert "0xC0000000, 0xD1FFFFFF, 0, 0x12000000" in asl
assert "0x0000FFFF, 0x02, Zero, 0x11" in asl
for device, gsis in (("RP05", [16,17,18,19]), ("RP08", [19,16,17,18])):
    body = asl.split("Device ("+device+")",1)[1].split("Device (",1)[0]
    routes = re.findall(r'Package \(\) \{ 0x0000FFFF, (Zero|One|0x02|0x03), Zero, (0x[0-9A-Fa-f]+) \}',body)
    assert [pin for pin,_ in routes] == ['Zero','One','0x02','0x03']
    assert [int(gsi,16) for _,gsi in routes] == gsis
assert "OperationRegion" not in re.sub(r"//[^\n]*", "", asl)
cmdline = re.search(r'ROOT_ZONE_CMDLINE: &str = "([^"]+)"', board)[1]
for required in ("modprobe.blacklist=nouveau", "module_blacklist=nouveau", "panic=0",
 "nvidia_drm.modeset=1 nvidia_drm.fbdev=1", "systemd.unit=graphical.target",
 "hvisor.gpu=graphics", "i2c_i801.disable_features=0x10", "lastbus=6,realloc=off"):
    assert required in cmdline
assert all(x not in cmdline for x in ("nomodeset", "maxcpus=", "nosmp"))
assert 'earlycon=efifb' not in cmdline
assert "pub const ROOT_ZONE_CPUS: u64 = 0x33;" in board
assert any(r["physical_start"] == 0x870000000 and r["size"] == 0xf000000 for r in regions)
print("PASS: native BDFs, dependency inventory, identity policy, RAM/ACPI contracts")
