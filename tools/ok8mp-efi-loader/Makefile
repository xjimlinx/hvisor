CLANG ?= clang
CFLAGS := --target=aarch64-unknown-windows -ffreestanding -fshort-wchar -fno-stack-protector -fno-asynchronous-unwind-tables -nostdlib -O2 -Wall -Wextra -Werror
LDFLAGS := -fuse-ld=lld -Wl,/subsystem:efi_application -Wl,/entry:efi_main -Wl,/nodefaultlib

.PHONY: all verify
all: build/hvisor-usb-r4-fast.efi

build:
	mkdir -p $@

build/hvisor-usb-r4-fast.efi: hvisor_loader.c | build
	$(CLANG) $(CFLAGS) -DPROFILE_USB $(LDFLAGS) $< -o $@

verify: all
	grub-file --is-arm64-efi build/hvisor-usb-r4-fast.efi
	grub-script-check bootstrap.cfg
	grub-script-check menuentry.cfg
