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
assert "modprobe.blacklist=nvidia,nouveau" in board
assert "module_blacklist=nvidia,nvidia_uvm,nvidia_modeset,nvidia_drm,nouveau,snd_hda_intel" in board
assert "panic=0" in board
assert "systemd.unit=multi-user.target" in board
assert "0x001AFFFF, One, Zero, 0x11" in asl
assert "0x001AFFFF, 0x02, Zero, 0x12" in asl
assert "OperationRegion" not in re.sub(r"//[^\n]*", "", asl)
devices = re.findall(r"pci_dev!\((\d+), (\d+), (0x[0-9a-f]+), (\d+) => (\d+), (0x[0-9a-f]+), (\d+),", board)
physical = [tuple(map(number, d[:4])) for d in devices]
virtual = [(number(d[0]), *map(number, d[4:])) for d in devices]
assert len(devices) == 6
assert len(set(physical)) == len(physical), "physical device assigned twice"
assert len(set(virtual)) == len(virtual), "guest PCI BDF collision"
print("PASS: GP102/HDMI BDFs, BARs, ACPI windows, GPA non-overlap, staged driver policy")
