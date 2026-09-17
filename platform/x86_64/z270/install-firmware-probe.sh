#!/bin/bash
# Run only from a dedicated candidate directory containing hvisor and the cfg.
# Does not select a boot entry, reboot, replace stable binaries or load a module.
set -euo pipefail
cd -- "$(dirname -- "$0")"
test "$(id -u)" = 0
test "$(findmnt -n -o UUID /boot)" = C75E-ADC9
test ! -e /boot/grub/custom.cfg
test ! -e /boot/hvisor/z270-firmware-probe
test ! -e grubenv.before
grep -q '^GRUB_DEFAULT=0$' /etc/default/grub
grep -q 'source .*custom.cfg' /boot/grub/grub.cfg
grub-script-check /boot/grub/grub.cfg
grub-script-check firmware-probe-grub.cfg
grub-file --is-x86-multiboot2 hvisor
for payload in boot.bin setup.bin vmlinux.bin initrd.img; do
    test -s "/boot/hvisor/z270/$payload"
done
sha256sum /boot/hvisor/z270/* > stable-payloads.before.sha256
cp -a /boot/grub/grubenv grubenv.before
cp -a /boot/grub/grub.cfg grub.cfg.before
install -d /boot/hvisor/z270-firmware-probe
install -m 0644 hvisor /boot/hvisor/z270-firmware-probe/hvisor
install -m 0644 firmware-probe-grub.cfg /boot/grub/custom.cfg
cmp hvisor /boot/hvisor/z270-firmware-probe/hvisor
sha256sum -c stable-payloads.before.sha256
sync
echo 'Installed additive probe entry; no boot selection or reboot performed.'
