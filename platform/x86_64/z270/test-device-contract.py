#!/usr/bin/env python3
"""Static GP102 board/ACPI contract checks; not a hardware passthrough test."""
import re
from pathlib import Path

root = Path(__file__).resolve().parent
board = (root / "board.rs").read_text()
asl = (root / "minimal-dsdt.asl").read_text()

def number(value):
    return int(value.replace("_", ""), 0)

regions = []
for block in re.findall(r"HvConfigMemoryRegion\s*\{([^}]+)\}", board):
    values = {}
    for name in ("physical_start", "virtual_start", "size"):
        values[name] = number(re.search(name + r"\s*:\s*(0x[\da-fA-F_]+|[0-9]+)", block)[1])
    regions.append(values)

windows = [(0xde000000, 0x1000000), (0xc0000000, 0x10000000),
           (0xd0000000, 0x2000000), (0xdf080000, 0x4000)]
for start, size in windows:
    matches = [r for r in regions if r["virtual_start"] == start]
    assert len(matches) == 1 and matches[0]["physical_start"] == start
    assert matches[0]["size"] == size
    for other in regions:
        if other is matches[0]:
            continue
        assert not (start < other["virtual_start"] + other["size"]
                    and other["virtual_start"] < start + size), "overlapping GPA"
    expected = f"0x{start:X}, 0x{start+size-1:X}, 0, 0x{size:X}"
    assert expected in asl, f"missing ACPI window {expected}"

for function in (0, 1):
    assert f"pci_dev!(0, 1, 0x00, {function} => 0, 0x1a, {function}," in board
assert "modprobe.blacklist=nouveau" in board
assert "module_blacklist=nouveau" in board
assert "nvidia_drm.modeset=1 nvidia_drm.fbdev=1" in board
assert "nomodeset" not in board
assert "panic=0" in board
assert "systemd.unit=graphical.target" in board
assert "hvisor.gpu=graphics" in board
assert "pub const ROOT_ZONE_CPUS: u64 = 0xff;" in board
cmdline = re.search(r'ROOT_ZONE_CMDLINE: &str = "([^"]+)"', board)[1]
assert "maxcpus=" not in cmdline and "nosmp" not in cmdline
assert any(r["physical_start"] == 0x700000000 and
           r["size"] == 0x16f000000 for r in regions)
for i, region in enumerate(regions):
    for other in regions[i+1:]:
        for key in ("physical_start", "virtual_start"):
            assert not (region[key] < other[key] + other["size"] and
                        other[key] < region[key] + region["size"]), "region overlap"
assert "0x001AFFFF, One, Zero, 0x11" in asl
assert "0x001AFFFF, 0x02, Zero, 0x11" in asl
assert "OperationRegion" not in re.sub(r"//[^\n]*", "", asl)
devices = re.findall(r"pci_dev!\((\d+), (\d+), (0x[0-9a-f]+), (\d+) => (\d+), (0x[0-9a-f]+), (\d+),", board)
physical = [tuple(map(number, d[:4])) for d in devices]
platform_reserved = {(0, 0, 0x1f, 0), (0, 0, 0x1f, 2),
                     (0, 0, 1, 0), (0, 0, 0x1b, 0),
                     (0, 0, 0x1c, 0), (0, 0, 0x1c, 4),
                     (0, 0, 0x1c, 7), (0, 0, 0x1d, 0)}
assert not platform_reserved.intersection(physical), "unsafe platform/bridge passthrough"
virtual = [(number(d[0]), *map(number, d[4:])) for d in devices]
assert len(devices) == 11
assert "pci_dev!(0, 0, 0x16, 0 => 0, 0x16, 0," in board
assert "pci_dev!(0, 0, 0x1f, 4 => 0, 0x1e, 0," in board
assert "i2c_i801.disable_features=0x10" in cmdline
assert "pci_dev!(0, 0, 0x1f, 3 => 0, 0x1d, 0," in board
assert "pci_dev!(0, 4, 0x00, 0 => 0, 0x1c, 0," in board
assert "0xDF200000, 0xDF207FFF, 0, 0x8000" in asl
assert "pci_dev!(0, 5, 0x00, 0 => 0, 0x1b, 0," in board
assert "0xDF100000, 0xDF103FFF, 0, 0x4000" in asl
assert len(set(physical)) == len(physical), "physical device assigned twice"
assert len(set(virtual)) == len(virtual), "guest PCI BDF collision"
print("PASS: GP102/HDMI BDFs, BARs, ACPI windows, GPA non-overlap, staged driver policy")
