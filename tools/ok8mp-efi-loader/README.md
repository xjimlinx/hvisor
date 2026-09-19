# OK8MP-C GRUB → Hvisor milestone (2026-09-19)

用户实测：R4 能通过 GRUB 启动 Hvisor/Linux 并登录串口；R4-fast 的
EFI → Hvisor 等待明显缩短。没有记录精确耗时，也未完成长期稳定性测试。
本里程碑针对 HDMI + USB Host、Root Linux CPU0–1；不宣称 GPU/Type-C
已验证。裸机 Arch 保持默认项，旧 R4 和 U-Boot bootm 保留为回退。

## Build

在本目录运行 `make verify`，需要 clang、lld、grub-file 和 grub-script-check。
输出 `build/hvisor-usb-r4-fast.efi`，复制到板子的
`/boot/hvisor-profiles/hdmi-usb-r1/hvisor-loader-r4-fast.efi`。
不要覆盖已有可用 EFI 文件。将 `menuentry.cfg` 的入口追加到现有
`/boot/grub/grub.cfg`，保留默认裸机入口及默认值。

加载器还需要这些已部署文件（不在本目录分发 Linux/DTB 二进制）：

- `/boot/mainline/OK8MP-C-mainline.dtb`
- `/boot/mainline/Image-7.2`
- `/boot/hvisor-profiles/hdmi-usb-r1/hvisor.bin`
- `/boot/hvisor-profiles/hdmi-usb-r1/zone0.dtb`

Hvisor/Zone0 源码和构建说明见本仓库 HDMI/USB 里程碑 `3cbc017`。

## Avoid stale embedded GRUB menus

原 BOOTAA64.EFI 内嵌完整旧菜单，单改磁盘 grub.cfg 不生效。
改为嵌入 `bootstrap.cfg`，按文件系统标签 IMX8MP_ARCH72 查找并加载
磁盘菜单。在板子上、本目录中生成候选文件：

```sh
grub-mkstandalone -O arm64-efi -o BOOTAA64.candidate.EFI \
  --modules='normal configfile linux fdt part_msdos ext2 search search_label terminal echo reboot halt chain' \
  --fonts=unicode 'boot/grub/grub.cfg=bootstrap.cfg'
grub-file --is-arm64-efi BOOTAA64.candidate.EFI
```

先备份 `/boot/efi/EFI/BOOT/BOOTAA64.EFI` 和磁盘菜单，再安装候选文件，
sync 后手动启动测试。不要直接运行会覆盖磁盘菜单的旧部署脚本。
GRUB 仍使用串口；此变更不提供 EFI 阶段的 HDMI 显示。

## Handoff and performance

1. 要求 EL2 和 identity mapping；固定目标区使用 AllocateAddress。
2. 固件把 0x94400000..0x140000000 标为 BootServicesData；R2 因此
   无法申请 0xa0000000。只对完整落在非 runtime 类型 4 区域的客体
   DTB/内核允许暂存，且检查加载器、栈和缓冲区重叠。
3. R3 固定申请 128 MiB 暂存内核导致 EFI_OUT_OF_RESOURCES。
   R4 按实际文件大小向上对齐到 4 KiB；实测内核 43424256 字节。
4. ExitBootServices 成功后清理数据缓存、关闭 MMU，搬运到
   DTB 0xa0000000、内核 0xa0400000，然后跳转 Hvisor 0x40400000。
5. R4-fast 将内核逐字节搬运替换为每轮 32 字节（四次对齐 64-bit
   load/store），余数按 8 字节及单字节处理。无 SIMD、不使用栈；
   地址保护及 EFI/cache 交接顺序不变。小 DTB 仍逐字节复制。

这是 Cortex-A53/OK8MP-C 专用加载器，不是通用 EFI 引导器。镜像头检查
不等于签名认证；不支持在任意固件、CPU 或内存布局中直接使用。
本地格式/语法检查不能替代实机验证；本次只有用户报告的启动与提速结果。

实机已部署 Host R4-fast SHA256：
`01013d30d126fc0c06190f5806622a3f7c9b5ad228b76aff82d9966f8c77d42e`。
PE 时间戳等构建信息可能使重新构建的二进制哈希不同。
