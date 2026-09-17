#!/bin/bash
# ISO boot baseline only: this does NOT run hvisor and proves no hvisor support.
# No guest disk, no networking, no host passthrough, no unattended installation.
set -euo pipefail
iso=${1:?Usage: windows-qemu-baseline.sh ISO NEW_OUTPUT_DIRECTORY}
out=${2:?Supply a new output directory}
test -f "$iso"
test ! -e "$out"
test -r /dev/kvm
test -w /dev/kvm
mkdir -p -- "$out"
out=$(realpath -- "$out")
iso=$(realpath -- "$iso")
cp /usr/share/edk2/x64/OVMF_VARS.4m.fd "$out/OVMF_VARS.fd"
exec qemu-system-x86_64 \
  -name 'Windows ISO baseline - NOT hvisor' \
  -machine q35,accel=kvm -cpu host -smp 4 -m 6144 \
  -drive if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd \
  -drive "if=pflash,format=raw,unit=1,file=$out/OVMF_VARS.fd" \
  -drive "file=$iso,format=raw,media=cdrom,readonly=on" \
  -boot order=d,menu=on -nic none -vga std \
  -display gtk -monitor none -serial "file:$out/serial.log" \
  -qmp "unix:$out/qmp.sock,server=on,wait=off" \
  -pidfile "$out/qemu.pid" -D "$out/qemu.log"
