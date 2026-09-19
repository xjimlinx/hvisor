# OK8MP-C EFI loader — current profile

唯一目标是 `desktop-2g5`；加载器使用已实测的 R4-fast 交接。
详见 [板级指南](../../docs/forlinx-ok8mpc.md) 和
[内存布局](../../docs/forlinx-ok8mpc-memory.md)。

## Build and deploy

运行 `make verify`，输出 `build/hvisor-desktop-2g5.efi`。
在板上安装为 `/boot/hvisor-profiles/desktop-2g5/hvisor-loader.efi`。
同目录必须有匹配的 Hvisor 和 Zone0 DTB，另需
`/boot/mainline/Image-7.2` 和 `/boot/mainline/OK8MP-C-mainline.dtb`。
先保存旧文件并校验上传，成功启动后再归档旧版。

`grub.cfg` 是两项菜单完整配置；`menuentry.cfg` 是当前 Zone0 的单项片段。
`bootstrap.cfg` 只按标签 IMX8MP_ARCH72 读取磁盘菜单，不内嵌旧菜单。
如需重建 GRUB，在板上本目录运行：

```sh
grub-mkstandalone -O arm64-efi -o BOOTAA64.candidate.EFI \
  --modules='normal configfile linux fdt part_msdos ext2 search search_label terminal echo reboot halt chain' \
  --fonts=unicode 'boot/grub/grub.cfg=bootstrap.cfg'
grub-file --is-arm64-efi BOOTAA64.candidate.EFI
```

确认候选有效并备份后才替换 BOOTAA64.EFI。当前已安装版本无需重建。
GRUB 保持串口显示，不承诺 EFI 阶段 HDMI 输出。

## Handoff design and limits

检查 EL2、identity mapping、地址重叠和镜像头。固定目标区域先申请；
仅对完整落入非 runtime BootServicesData 的客体 DTB/内核使用暂存。
按实际文件大小申请页，成功 ExitBootServices 后清缓存、关闭 MMU，
再搬运至 0xa0000000 / 0xa0400000，并跳转 0x40400000 的 Hvisor。

内核每轮 32 字节对齐标量复制，余数按 8/1 字节；无 SIMD，不依赖栈。
用户已验证比逐字节搬运明显快，未记录精确耗时。

这不是通用 EFI 引导器，也不提供镜像签名认证。旧失败迭代、旧板型、
旧菜单可从 Git 历史恢复，不再维护多套可选配置。
