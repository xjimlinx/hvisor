# HDMI + USB Host R1 — 2026-09-19

HDMI R2: user confirmed physical display output. SSH confirmed native TX
initializes without the old pinctrl conflict. Long-term stability untested.

USB R1: user confirmed boot; SSH verified USB2/USB3 root hubs, Cypress
hub, Exar UART and wireless keyboard/mouse receiver enumeration. Receiver
first enumeration timed out, then succeeded; long-term stability untested.
Separate profile `/boot/hvisor-profiles/hdmi-usb-r1/`; R2 preserved.
No GRUB/default boot changes, no automatic reboot.

R2 still had legacy USB dependencies; both imx8mp-dwc3 controllers failed
with deferred-probe timeout. This candidate imports mainline HSIO block
controller, GPC HSIO/USB PHY power domains and the PCIe PHY domain required
by the HSIO driver's complete provider registration. This does not enable
the PCIe controller. USB wrappers, PHYs and USB Host VBUS pin group come
from the board's mainline DTB; source phandles are remapped structurally.
USB0 remains disabled as in that DTB; USB1 is host mode. Type-C/OTG role
switching is not implemented. Optional NoC interconnect configuration is
omitted, retaining firmware policy as in the HDMI candidate.

Generator: `tools/ok8mp_usb_dts.py`, inputs HDMI R2 DTB and the board's
mainline DTB. Structural assertions verified all HDMI nodes, CPUs, chosen,
RAM/reserved memory and AIPS3 remain identical. Existing Hvisor MMIO and
INTIDs already cover USB/HSIO (USB IRQs 72/73 and wake IRQs 180/181);
no additional interrupt assignments or Hvisor rebuild were required.

SHA256:
- zone0.dtb: `2a9a4f0198737e2536588429fc4389f8b8d2b79cae132ff0a636264f6b01bc62`
- hvisor.bin: `37b6f3dd99f3d0012d4a375a97ea4807e4a35f344642bbce609fdc3e546fe958`

Desktop `forlinx` has an appended single-line HDMI USB R1 command.
After manual boot: confirm HDMI and SSH, inspect xHCI/root hubs with lsusb,
then enumerate a keyboard or flash drive on the Host port; do not write or
format a drive as a test. Check dmesg for power-domain failures/MMIO faults.
AP, audio and CPU-frequency pending probes are outside this change.

## Rebuild the milestone without temporary input files

The complete deployed device-tree source is checked in at
`platform/aarch64/forlinx-ok8mpc-hdmi/zone0-hdmi-usb-r1.dts`.
Compile with `dtc -I dts -O dtb -o zone0.dtb` followed by that path.
Build Hvisor with `make BID=aarch64/forlinx-ok8mpc-hdmi gen_cargo_config`
then `make BID=aarch64/forlinx-ok8mpc-hdmi target/aarch64-unknown-none/release/hvisor.bin`.
The milestone includes the existing local PSCI/SMCCC compatibility changes
used by the tested binary; it is not a claim that every PSCI path is tested.
