# Z270 bare-metal bring-up snapshot

Local ASUS PRIME Z270-AR / i7-7700K adaptation, not a generic PC profile.
Firmware addresses, BAR windows and root UUID in board.rs are machine-specific.

## Build

Requires the repository Rust toolchain and ACPICA `iasl` in PATH:

```sh
make ARCH=x86_64 BOARD=z270 MODE=release LOG=info elf
```

Build-time ASL compilation supplies the minimal guest DSDT. Zone0 exposes one
CPU, the host bridge, SATA AHCI, Intel PCH xHCI, I219-V and GP102 with HDMI audio;
other devices remain unassigned. Guest Linux payloads are the existing Arch 6.18.50-2-lts files,
not included here. Keep their initrd length consistent with board.rs.

## Hardware verification, 2026-09-13

- Guest journal confirmed the existing root and data filesystems mounted and
  userspace services started.
- After quiescing assigned xHCI before the VT-d address-space switch, the user
  confirmed that the physical USB keyboard works.
- Network follow-up was verified over IPv6 SSH inside Zone0: physical I219-V
  `00:1f.6` appears as guest `00:19.0`, uses `e1000e`, and connects through the
  existing NetworkManager PPPoE profile on `enp0s25`. The profile has no fixed
  interface-name/MAC binding; no credentials belong in this repository.
- Tested network-enabled ELF SHA256:
  `14eb68d78ebe2e458a99551a53b8c0e4df95636f6caa9a9962609d5e0f8e4ede`.
- This is a bring-up checkpoint, not proof of long-term storage integrity,
  complete interrupt virtualization or arbitrary PCI passthrough.

## Scope and known limits

### GPU candidate (not yet hardware-verified)

Physical `01:00.0/1` is presented as `00:1a.0/1`, keeping both functions in
Zone0. The physical upstream bridge stays firmware-owned and hidden; no bridge
reset or bus renumbering is requested. DMA contexts use physical requester IDs
`0100/0101`, not guest BDFs. Assigned GPU functions have MSI/INTx masked and bus
mastering cleared before the existing DMA drain/VT-d switch; memory decoding
and firmware scanout remain enabled. This is not a full GPU reset protocol.

Native inventory: GP102 BAR0 `de000000/01000000`, BAR1 `c0000000/10000000`,
BAR3 `d0000000/02000000`, I/O `e000/80`, HDMI BAR0 `df080000/4000`.
These windows are mirrored in minimal ACPI; EPT maps memory windows UC for
conservative bring-up. PEG0 AR01 INTx routes A-D are GSI 16-19. Firmware BAR
relocation is not supported by this fixed board profile.

The first GPU candidate failed before producing a journal; its photo shows
an unhandled VM exit on parking CPUs, but not the reason. Live native BARs
still match the mappings. The prior network guest journal proves nvidia loaded
despite modprobe.blacklist, and native modules-load configuration requests
nvidia-uvm. Thus the intended driver-free test was not actually isolated.

This candidate uses kernel `module_blacklist` for the entire NVIDIA family,
nouveau and snd_hda_intel, plus `panic=0`. It tests enumeration only: after
reboot verify SSH/root/USB/network, `lspci -nnvv -s 00:1a`, and absence of
these modules. Manual modprobe is also blocked for this boot. A later driver
candidate must remove this kernel blacklist and control the userspace loaders
before manually testing the installed proprietary 580xx driver.
Acceptance requires `nvidia-smi`, GPU work and clean DMA/IRQ diagnostics;
PCI enumeration or compilation alone is not success. No driver upgrade,
desktop/KMS takeover, GPU reset, or automatic reboot is part of this candidate.
The hvisor framebuffer console shares the GPU: do not enable KMS/desktop
takeover before providing an independent hvisor logging channel.

Normal Arch/GRUB and rollback boot files are maintained separately; this repo
does not contain deployment credentials or boot artifacts. Wi-Fi remains
unassigned; GPU support is experimental as described above. Guest APIC state supports the
tested single-vCPU path, not full nested interrupt priority/level-triggered
semantics or multi-vCPU xAPIC logical routing. The MMIO decoder is a limited
MOV-family emulator, not a general x86 instruction emulator. Early xHCI tracing
remains enabled in the diagnostic guest command line.

Pure regression tests are embedded in `acpi_image.rs`, `hpet_time.rs`,
`mmio_address.rs`, `apic_destination.rs` and `apic_registers.rs`. They can be
compiled independently with `rustc --edition=2021 --test <source> -o <binary>`;
passing them does not replace testing on this physical board.
