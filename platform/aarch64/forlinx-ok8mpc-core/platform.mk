$(hvisor_bin): elf
	$(OBJCOPY) $(hvisor_elf) --strip-all -O binary $(hvisor_bin).tmp
	mkimage -n hvisor_img -A arm64 -O linux -C none -T kernel -a 0x40400000 \
		-e 0x40400000 -d $(hvisor_bin).tmp $(hvisor_bin)
	rm -f $(hvisor_bin).tmp
