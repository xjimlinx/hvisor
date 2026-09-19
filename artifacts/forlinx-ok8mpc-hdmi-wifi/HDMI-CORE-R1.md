# HDMI core R1 — 2026-09-19

Status: R1 boots to serial login and SSH; HDMI probe fails due to pin ownership.
R2 deployed, checksum verified; R2 boot/display verification pending.

This candidate grafts mainline native-HDMI nodes onto the boot-tested core
DTB. CPU, chosen, RAM and the entire AIPS3 subtree (PMIC/I2C, SD, serial,
FEC) were compared byte-for-byte and are unchanged. Generation is implemented
in `tools/ok8mp_hdmi_dts.py`; input files were the boot-tested core DTB and
the board's `/boot/mainline/OK8MP-C-mainline.dtb`.

Imported dependencies: GPC with only HDMI mix/PHY domains, HDMI pin group,
block controller, irqsteer, LCDIF3, PVI, TX, PHY and connector. Imported
phandles are relocated and external clock/GIC references remapped to core.
PAI is omitted along with the TX audio endpoint. The local driver handles
absent/disabled PAI with a video-only probe.

Optional HDMI interconnect properties are omitted: this first candidate
retains firmware NoC configuration rather than enabling a new NoC/DDR
driver chain. NoC performance policy remains unvalidated. CMA is reduced to
256 MiB within 0xb0000000..0xd0000000; allocation must be checked after boot.

Hvisor uses the same resources and 36 INTIDs as core. Corrections to earlier
reports: DT SPI 43 maps to GIC INTID 75; GPIO1 SPI 64/65 maps to INTID 96/97.
Both were already covered. Adding INTIDs 43/65 was not a valid PMIC fix.
Also, earlier active-device scans wrongly traversed disabled parent buses;
they did not establish that their children were actually probing. The
core-mainline hang is unresolved; the final printed I2C line alone does not
identify the stuck driver.

Deployed under `/boot/hvisor-profiles/hdmi-core-r1/`:

- hvisor.bin: `37b6f3dd99f3d0012d4a375a97ea4807e4a35f344642bbce609fdc3e546fe958`
- zone0.dtb: `c3a8491b67b851f2f62a44989e16e5be1d7b3a70e08415be5ab25c1c9eba2d04`

Desktop `/home/xein/Desktop/forlinx` has an appended `HDMI Core R1` command.
The known booting `/boot/hvisor-profiles/core/` remains the recovery profile.
Pass criteria: SSH and SD work; no SError or Hvisor MMIO panic; CMA allocates;
native DRM HDMI connector appears and reports connected with the monitor.

## R1 hardware result and R2 pin ownership correction

SSH inspection on 2026-09-19 found the actual TX probe error:
`MX8MP_IOMUXC_HDMI_DDC_SCL already requested by 30330000.pinctrl; cannot claim for 32fd8000.hdmi`.
The HDMI TX driver exited with `-EINVAL` when applying pinctrl. This explains
the downstream PVI/LCDIF deferred probes; it is not proof of missing IRQs.
CMA successfully allocated 256 MiB at 0xc0000000. USB/SDMA deferred probes
remain unresolved and are not changed in R2.

R2 removes only mux offsets 0x240 (DDC SCL), 0x244 (DDC SDA), and 0x248
(CEC) from the old default hog, leaving native TX hdmigrp to own them.
The 0x24c HPD entry is retained. Structural comparison of the compiled DTBs
confirmed the sole changed property is pinctrl hoggrp `fsl,pins`; Hvisor,
CPU, RAM, clocks, SD, PMIC and other device properties are unchanged.

Deployed separately to `/boot/hvisor-profiles/hdmi-core-r2/`, preserving R1
and core recovery files. Hvisor hash remains the R1 hash above.
R2 zone0.dtb SHA256: `b668e876e20b5a14aaa805256fa055b61779640646497cc552519e3f6944f3f4`.
The single-line R2 U-Boot command is appended to the desktop `forlinx` file.
No default boot setting or GRUB entry was changed. Next verification:
boot R2, check TX binds without pinctrl conflict, DRM connector EDID/modes,
and actual monitor output. HDMI is not yet claimed working.
