# 原生 PCI 拓扑实机验证（2026-09-13）

## 已确认的回归原因

恢复 bus00–06 后，第一次候选只出现14个PCI功能，00:1f.*全部缺失。
`PciIterator::next_device_not_ok` 原来以退出节点的 subordinate_bus
等于扫描最大bus作为结束条件。Z270的空bus06位于00:1d.0桥下，退出
该子总线后根总线尚有00:1f.*未扫描，却被提前结束。
修复为：只有遍历栈真正清空才结束。新增测试抽取实际Rust方法，验证
退出最高子总线后继续经过00:1e、00:1f，以及根栈退出正常结束。

另一处归属判断将全部class06视为PCI桥；改为仅PCI-to-PCI 06:04，
保留独立host判断。LPC/ISA 06:01按普通拥有者路径处理。

## 实机结果与边界

修复版542b40505d7dab33ce56a2843c7a1344b837e31e4266273278bd43c9b47f7223
已实机重启：19个PCI功能全部可见、桥与端点使用原生BDF；
CPU0–7、MemTotal32714060kB、NVIDIA580.178.04显示启用、
plasmalogin active、两张声卡、根盘/dev/sdb2和数据盘/dev/sda1挂载正常。
I219-V恢复为enp0s31f6，PPPoE与Wi-Fi均在线，失败服务0，启动13.253秒。
LPC/PMC目前只确认枚举可见，不声称平台电源管理、看门狗已验证。

## 中断路由

最终版e5ecf63baf43bc564a25e3f2f45a0145934b983347a88e02804544dfab261eaa
已再次实机重启，boot_id=b039413c-fb08-46eb-8671-f8c29d75512b。
19功能、NVIDIA显示、plasmalogin、双网络正常，失败服务0；当次内核
日志未匹配no GSI、derive routing、Malformed early、DMAR fault或xhci error。
这证明本次启动检查通过，不替代负载、INTx fallback和平台电源功能测试。

保存的原生DSDT显示RP05._PRT使用AR08（A–D对应16/17/18/19），
RP08使用AR0B（19/16/17/18），已加入最小DSDT。
根桥slot01/1b/1c/1d及MEI/PCH USB按AR00补齐。
GPU的A/B引脚补充16/17。保留之前HDMI pin-C和SMBus pin-B的
已用fallback值17/16；它们与原生表部分描述存在差异，暂不声称已
验证INTx fallback。HDA/NIC等仍使用MSI，SMBus仍使用轮询。
移除内核明确报Malformed early option的earlycon=efifb，保留console与hvisor INFO。

## 回退

所有路径前缀：/root/arch-z270-maintenance/backups/

- hvisor-audit-20260913-DwVbAr：原11功能稳定版871150a1。
- hvisor-audit-20260913-CcgDmm：14功能回归版c2e46b54，**不是稳定回退点**。
- hvisor-audit-20260913-58G2tW：19功能已验证版542b4050。

每次安装都保留整个z270启动目录及GRUB配置，使用一次性实验启动项；
默认GRUB_DEFAULT=0（原生Arch）未修改。回退时需核验配套ELF/initrd哈希，
不能只按文件名推断版本。新的平台控制与跨Zone安全不在本轮验收范围。
