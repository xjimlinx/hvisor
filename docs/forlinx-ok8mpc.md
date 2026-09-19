# 飞凌 OK8MP-C：当前唯一配置

适用 Forlinx OK8MPlus-C / OK8MP-C（i.MX8MP），不是 Orange Pi。
已实测启动：GRUB → EFI R4-fast → Hvisor → Arch Linux ARM 7.2。

## 当前状态

- Zone0 使用 CPU0–1，三段 RAM 合计 2.5 GiB，系统 MemTotal 约 2.3 GiB。
- HDMI、USB Host、串口和 SSH 可用。
- Etnaviv GC7000/GC520 已绑定，存在 renderD128；未进行定量图形性能测试。
- 暂无 swap；扩容不保证所有负载都不会 OOM。
- USB-C 已验证 Host 枚举：扩展坞、HID 接收器、RTL8153 网卡（USB 2.0 480M）。
- 板载无线已通过 SDIO 枚举、Wi-Fi 关联、DHCP 和无线 HTTPS 联网测试。
- Type-C SuperSpeed、两种插入方向、Gadget 和长期稳定性仍待验收；无线热点 AP 模式及蓝牙未验收。
- Type-C 不等于视频输出：当前未实现 DP Alt Mode，请使用板载 HDMI。
- 保留上游其他平台代码。旧 OK8MP-C 实验方案从工作树移除，可从 Git 历史恢复。

## 构建入口

唯一板型：`platform/aarch64/forlinx-ok8mpc-desktop/`。
完整设备树：同目录 `zone0.dts`，无需依赖 /tmp 中的旧 DTB。

```sh
make BID=aarch64/forlinx-ok8mpc-desktop gen_cargo_config
make -j32 BID=aarch64/forlinx-ok8mpc-desktop target/aarch64-unknown-none/release/hvisor.bin
mkdir -p target/ok8mpc
dtc -I dts -O dtb -o target/ok8mpc/zone0.dtb platform/aarch64/forlinx-ok8mpc-desktop/zone0.dts
make -C tools/ok8mp-efi-loader verify
```

需要项目 Rust 工具链、交叉 binutils、mkimage、dtc、clang/lld 和 GRUB 工具。
[内存布局](forlinx-ok8mpc-memory.md)；[EFI 部署方法](../tools/ok8mp-efi-loader/README.md)。
USB-C 还需要针对匹配 Linux 构建安装 [GPIO 方向切换模块](../tools/ok8mp-typec-switch/README.md)，
仅复制 Hvisor/DTB 不足以复现完整 Type-C 链路。

## 板上固定路径

- 裸机内核/DTB：`/boot/mainline/Image-7.2`、`OK8MP-C-mainline.dtb`。
- 当前 Hvisor：`/boot/hvisor-profiles/desktop-2g5/` 下的
  `hvisor.bin`、`zone0.dtb`、`hvisor-loader.efi`。
- GRUB：`/boot/efi/EFI/BOOT/BOOTAA64.EFI` 读取 `/boot/grub/grub.cfg`。
- 菜单只保留裸机 Arch（默认）和当前 Zone0。不再列出实验或旧回退项。

2026-09-19：用户确认 Type-C 测试版能运行，授权将其 Hvisor/DTB 提升为
desktop-2g5 唯一版本；GRUB EFI 加载器本身不变，菜单仍只有两项。
此次未重启验收 GRUB 新组合，不能把之前的测试版直接启动等同于本轮 GRUB 验收。
SDMA、AUD2HTX 时钟依赖及 cpufreq 的既有问题仍保留为已知限制。
旧 boot 实验文件先备份到主机，再从板上移除；系统包管理的 Image、
dtbs、initramfs、用户文件、网络配置、模块、固件均不在删除范围。
当前系统是开发 rootfs，不是已去身份化的可发布镜像。
