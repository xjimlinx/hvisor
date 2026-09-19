# OK8MP-C 板载 SDIO 无线模组

2026-09-19 更新：测试 DTB 已通过无线关联、DHCP（wlan0）、HTTPS 联网。
用户随后通过旧 GRUB 第二项启动，因该项仍引用旧 DTB 而未加载无线配置。
现将已测 DTB 归并到 desktop-2g5，GRUB 第二项增加 Wi-Fi 标识，仍只保留两项。
本轮归并后的 GRUB 启动尚待用户下一次重启确认。无线热点 AP 模式与蓝牙未验收。

以下保留初始排查与测试流程记录，测试路径不再是当前启动入口。

## 本轮证据与修改

- 当前系统没有 SDIO 设备、无线网卡，Zone0 DTS 缺少 USDHC1 控制器节点。
- GPIO2_IO8 被 mmc-pwrseq 占用，调试输出显示物理低电平（复位有效）。
- 现有系统已安装 mwifiex_sdio 驱动与 Marvell 固件；此前裸机日志曾成功
  下载 mwifiex 固件并关联网络。不要仅凭“AP 模组”的称呼改用 Broadcom 驱动。
- 按厂商 OK8MP-C DTS 补 30b40000 USDHC1、4-bit SDIO、GPIO2_IO8 复位。
  时钟编号参考匹配的主线 imx8mp 节点，使用当前树 CCM phandle 3。
- 复位 pinctrl 放在 pwrseq 节点，由其在取 GPIO 前设置 GPIO 复用。
- INTID54 和 AIPS3 MMIO 已被 Zone0 分配，无需修改 board.rs 或扩大 IRQ 列表。
- 固定 mmc0 为无线，保留 SD 根分区 mmc2 别名，避免再次改变 root 设备编号。
- 不启用整套旧设备树、不改 PMIC、内存、HDMI、USB 或 CPU 数量。

## 测试启动

仅新增板上 `/boot/zone0-wlan-test.dtb`，Hvisor/内核/板级 DTB 仍用当前版本。
在 U-Boot 使用通常的 desktop-2g5 命令，只将 Zone0 DTB 路径改为此文件。
不要 saveenv。测试后回到原 GRUB 项即可回退。

## 验收

1. `ls /sys/bus/sdio/devices` 和设备 vendor/device 信息确认实际型号。
2. `dmesg` 确认 SDIO 枚举、固件激活，无 mmc 超时或中断风暴。
3. `nmcli device` 出现无线接口；使用 NetworkManager 扫描并连接已有网络。
   不输出已有连接的密码；不新建开放热点、不修改当前有线 SSH 路由。
4. 验证 DHCP、连通性和持续传输，再检查 USB-C、HDMI、根分区均未回归。
5. 若需要热点（AP 模式），另外检查 `iw phy` 的 supported interface modes，
   并测试客户端关联；模组正常不代表热点功能已经验收。蓝牙亦为独立任务。

当前检查：dtc 编译成功，2.5 GiB 内存 banks 和 mmc2 根分区路径未改变。
正式 DTB SHA256：`24cc4745c37c4de453c7e9ab2d1db1b6f3f890087a982f18d2aaeef363910c51`。
