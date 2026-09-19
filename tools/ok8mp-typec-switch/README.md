# OK8MP-C Type-C support

## Current baseline (2026-09-19)

Promoted to the existing `desktop-2g5` profile at the user's request after
boot and USB host enumeration tests. FUSB302 and the orientation module bind;
TCPM reports host/source. The dock, HID receiver and RTL8153 enumerate at
480 Mbit/s. Existing USB-A devices remain enumerated. No SuperSpeed peripheral,
both-orientation test or gadget function has been validated yet. The 63-second
detach/reattach in the supplied log has not been independently attributed to
manual unplugging; do not claim long-term connection stability.

The stable EFI loader remains unchanged and loads the updated images from the
same `desktop-2g5` path. Only two GRUB entries are retained. The temporary
`typec-test` directory is archived off-board and removed after promotion.

Forlinx documents this port as USB3 dual-role and lists HDMI separately:
https://docs.forlinx.net/nxp/okmx8mpq-c/OK-MX8MPQ-C_User_Hardware.html
Together with the vendor DTS USB-only routing, there is no evidence for DP Alt
Mode wiring. Ordinary USB-C HDMI/DP Alt Mode adapters are not supported by this
implementation. Use the board HDMI connector. USB graphics adapters are a
separate driver/hardware project, not an enabled feature here.

## Initial candidate record

Status: compiled and deployed as a candidate, awaiting a boot and physical connector tests. Do not label
Type-C as verified based on the existing USB-A host test.

2026-09-19 checks: Hvisor and module build succeeded; DTB compiled (inherited
numeric-phandle warnings remain); DTB memory banks match the baseline.
The installed module loaded on the running 7.2.0-arch72 kernel without an ABI
error, but the old live DT has no switch device, so this is not a probe test.
Local and deployed SHA-256 hashes match:

```text
f51dca20d4d3fb1239b2858848097a8cc3df232cdc39d7f267920986d0192e32  hvisor.bin
e5b08c000539d78bb5784c53d0e37324297b42ea994c1a0a23e552a5a8d7109e  zone0.dtb
9f4557071d70e1157a6007532aedbd4a5433a35684b540dfa1a37c520a40b69f  ok8mp-typec-switch.ko
```

The board DTS enables USB0 in dual-role mode and FUSB302 on I2C3 (0x22).
The vendor OK8MP-C DTS assigns VBUS to GPIO1_IO14, interrupt to GPIO4_IO19,
and SuperSpeed orientation to GPIO4_IO20. The old USB-A pinmux assignment of
GPIO1_IO14 as USB2_PWR conflicts with this wiring and is removed. TCPM controls
VBUS; it must not be forced always-on when connected to a PC.

This GPL module ports NXP's vendor `drivers/usb/typec/mux/gpio-switch.c`
(2019, Jun Li) to the current `typec_switch_dev` API. It intentionally retains
`nxp,cbtl04gp` and vendor GPIO polarity rather than identifying a USB3 switch
as an unrelated SBU/DisplayPort switch. No DP Alt Mode is implemented.
Reference: Forlinx vendor `OK8MP-linux-kernel/drivers/usb/typec/mux/gpio-switch.c`
and `arch/arm64/boot/dts/freescale/OK8MP-C.dts` supplied with this board.

Build against the **exact configured/built kernel** running on the board:

```sh
make -C /path/to/linux-7.2 -j32 ARCH=arm64 LLVM=1 M="$PWD/tools/ok8mp-typec-switch" modules
modinfo tools/ok8mp-typec-switch/ok8mp-typec-switch.ko
```

Install the module in `/usr/lib/modules/7.2.0-arch72/extra/`, run
`depmod -a 7.2.0-arch72`. Udev loads it by its OF alias when the new DTS is
booted; no global modules-load setting is necessary. An out-of-tree module
adds the kernel's `O` taint flag, which is expected, not a machine check.

Candidate deployment uses `/boot/hvisor-profiles/typec-test/{hvisor.bin,zone0.dtb}`.
The validated `/boot/hvisor-profiles/desktop-2g5/` files and GRUB menu are
not overwritten. A one-off U-Boot boot tests the candidate without `saveenv`.
After validation, consolidate into the existing desktop profile and use its
existing GRUB loader; do not keep an extra permanent menu entry.

## Hardware acceptance

1. Boot initially with Type-C disconnected; verify SSH, HDMI, GPU, both CPUs,
   2.5-GiB banks and the existing USB-A keyboard/hub still work.
2. Check `dmesg`, `/sys/kernel/debug/devices_deferred`, `/sys/class/typec/`,
   `/sys/class/usb_role/`, `/sys/class/udc/`, and I2C `2-0022/driver`.
   FUSB302, orientation switch and DWC3 must bind without deferred timeout.
3. Use a Type-C OTG adapter and known USB device; test both plug orientations,
   enumeration with `lsusb -t` and a read-only device operation. Test USB2 and
   SuperSpeed separately. Do not write raw disks.
4. Connect to a PC with a known data cable. Check device/sink role. UDC presence
   alone does not expose a USB function to the PC: a separately configured
   ACM or Ethernet gadget is needed for end-to-end gadget verification.
   Never export the mounted root filesystem as a mass-storage gadget.
5. Test disconnect/reconnect and verify no interrupt storm or USB-A regression.

If unsuccessful, reboot the original Hvisor GRUB entry; no firmware environment
changes are needed. Keep logs and do not broaden changes to AP, PMIC, memory,
HDMI, or GPU while isolating Type-C failures.
