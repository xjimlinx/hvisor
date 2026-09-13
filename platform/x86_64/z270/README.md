# Z270 bare-metal bring-up snapshot

Local ASUS PRIME Z270-AR / i7-7700K adaptation, not a generic PC profile.
Firmware addresses, BAR windows and root UUID in board.rs are machine-specific.

## Build

Requires the repository Rust toolchain and ACPICA `iasl` in PATH:

```sh
make ARCH=x86_64 BOARD=z270 MODE=release LOG=info elf
```

Build-time ASL compilation supplies the minimal guest DSDT. Zone0 exposes one
CPU, the host bridge, SATA AHCI and Intel PCH xHCI; other devices remain
unassigned. Guest Linux payloads are the existing Arch 6.18.50-2-lts files,
not included here. Keep their initrd length consistent with board.rs.

## Hardware verification, 2026-09-13

- Guest journal confirmed the existing root and data filesystems mounted and
  userspace services started.
- After quiescing assigned xHCI before the VT-d address-space switch, the user
  confirmed that the physical USB keyboard works.
- Tested deployed ELF SHA256:
  `10c3885b3a3912ecff31981a218f31fe43f13f104739a98022eeafba30f6e3e6`.
- This is a bring-up checkpoint, not proof of long-term storage integrity,
  complete interrupt virtualization or arbitrary PCI passthrough.

## Scope and known limits

Normal Arch/GRUB and rollback boot files are maintained separately; this repo
does not contain deployment credentials or boot artifacts. No GPU/network
passthrough is enabled by this board profile. Guest APIC state supports the
tested single-vCPU path, not full nested interrupt priority/level-triggered
semantics or multi-vCPU xAPIC logical routing. The MMIO decoder is a limited
MOV-family emulator, not a general x86 instruction emulator. Early xHCI tracing
remains enabled in the diagnostic guest command line.

Pure regression tests are embedded in `acpi_image.rs`, `hpet_time.rs`,
`mmio_address.rs`, `apic_destination.rs` and `apic_registers.rs`. They can be
compiled independently with `rustc --edition=2021 --test <source> -o <binary>`;
passing them does not replace testing on this physical board.
