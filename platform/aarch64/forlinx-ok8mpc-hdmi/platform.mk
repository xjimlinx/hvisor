# Hardware deployment uses U-Boot on the board; this target only packages the
# already linked Hvisor ELF as the legacy arm64 image accepted by `bootm`.
$(hvisor_bin): elf
	$(OBJCOPY) $(hvisor_elf) --strip-all -O binary $(hvisor_bin).tmp
	mkimage -n hvisor_img -A arm64 -O linux -C none -T kernel -a 0x40400000 \
		-e 0x40400000 -d $(hvisor_bin).tmp $(hvisor_bin)
	rm -f $(hvisor_bin).tmp
