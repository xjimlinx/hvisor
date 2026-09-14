# Z270 设备归属与分 Zone 依赖清单

## 实机更新：19项原生拓扑已启动

最新结果与回归原因见 [NATIVE_TOPOLOGY.md](NATIVE_TOPOLOGY.md)。
已确认全部19项PCI功能、原生BDF、8线程、约31.2GiB RAM、GPU桌面、
两张声卡、根盘/数据盘和双网络在线。LPC/PMC仅确认可见，平台功能
与跨Zone隔离未验收。下文“尚未实机验证”段为本轮之前的历史候选记录。

## 历史候选说明：可信 Zone0 全设备（已由上方实机结果更新）

用户明确要求先将全部设备分配给 Zone0，因此下方历史记录中的
LPC/PMC/桥“暂不分配”不再是当前目标；它们的隔离风险仍然成立。
本地 `board.rs` 通过 `zone0_native_inventory!` 单一具名清单生成
19 项原生 BDF 分配，保留 bus 00–06。组号和父节点是审核元数据，
不是运行时隔离策略。原来 11 项功能的实机验证不能自动外推到新配置。

完整架构见 [SVG 框架图](ARCHITECTURE.svg)，分配接口的实现范围与
后续设计见 [PCI 分配设计](PCI_ASSIGNMENT.md)。图中硬件地址均为
物理 BDF；下方旧的扁平客体地址只用于理解已验证版本和回退。

新候选仍经 vPCI 配置空间处理，不表示所有桥/平台寄存器均已可写。
hvisor RAM、VMX、VT-d 及必要中断控制不作为普通设备交给 Zone0。
本次本地检查：release 编译、19 项清单/依赖/BAR 合同、4 项 PCI
扫描回归、DSDT 编译求值、ELF 与 Zone0 内存不重叠检查通过。

2026-09-13；物理清单来自原生 Arch PCI/ACPI/e820 采集。组号为该次
原生 Linux 的 IOMMU 分组，不等于 hvisor 已实现同等隔离。
当前只有 Zone0，不能把单 Zone 跑通解释为跨 Zone 安全验证。

## 本轮实测更新

继续接入结果：SMBus 00:1f.4→00:1e.0 已由 i801_smbus 接管，
`i2c_i801.disable_features=0x10` 使用轮询，未执行用户态总线扫描或
SPD/EEPROM 写操作。MEI 00:16.0→00:16.0 已由 mei_me 接管，
/dev/mei0 权限 root 0600；未执行管理命令或固件更新。
两项分别重启验证，plasmalogin active、失败服务0、网络地址重新确认。

当前保留项是 LPC 00:1f.0、PMC 00:1f.2 与六个桥，不作为直通清单
里的“漏项”。静态检查见 pci_struct.rs 的 VirtualPciAccessBits::bridge：
桥窗口/复位保护未完成；Linux 参考 lpc_ich_probe 会创建 GPIO/iTCO
子设备，pmc_core_probe 涉及额外平台电源 MMIO。未核准这些访问前
不开放其控制权；test-device-contract.py 对这些物理 BDF 增加拒绝断言。
若要客体查看完整拓扑，应实现只读虚拟描述；若要平台控制，需要专门
代理/访问白名单及跨 Zone 复位策略，不能仅再填一个 pci_dev!。

最新 MEI ELF：871150a1fabe0cb8da6e5ceacf9a86e639c3e5071b650414ad50775997993ae3。
部署前备份 hvisor-audit-20260913-gJtWDN（SMBus 已启用）。

- AX210 已分配为 00:1b.0：iwlwifi 固件加载、自动关联、DHCP 地址获取
  正常；绑定 wlan0 ping 网关 10.31.0.1 两次成功。管理机直连其 Wi-Fi
  IPv4 曾报无路由，不宣称该入站路径已通过。
- ASMedia 已分配为 00:1c.0：xhci_hcd 接管，新增 Bus003/004 的
  480M/10000M 根集线器；尚无插入该控制器的外设，实际传输待验证。
- PCH Audio 已分配为 00:1d.0：snd_hda_intel、HDA Intel PCH 和
  codec/模拟接口注册正常，与 HDMI 声卡并存；实际录放音待验证。
- 三项分别部署重启；最终桌面和网络在线，systemctl --failed 为0。
- 表中这三项“计划/待接入”是初始设计记录，以本段实测状态为准。
- 额外修复 PCI 桥扫描返回路径：恢复父功能后必须先遍历同槽后续
  function，否则跳过 00:1c.4/.7，造成 ASMedia/AX210 不可见。
- 最新 ELF e671624a25a78dfc6cffc1638b4d864e557487cac6a8f68838bb53ee6ed9ec8e。
  前一稳定版备份：hvisor-audit-20260913-stFcFV（ASMedia 已启用）。
- SMBus/MEI/PMC/LPC/桥仍未分配；不要把本轮结论用于跨 Zone DMA/IRQ
  安全证明。PCH 音频的组10限制仍然存在。

## 拓扑及所有权

| 物理节点 | 当前/计划客体 BDF | 原生组 | 上游/共享依赖 | 状态及分离约束 |
|---|---|---|---|---|
| 00:00.0 host bridge | 00:00.0 | 0 | 整个平台 | Zone0；不交给不可信 Zone |
| 00:14.0 PCH xHCI | 00:14.0 | 2 | 全部下游 USB、AX210 蓝牙 USB 功能 | 已验证键鼠；分离整个控制器会带走键鼠/蓝牙 |
| 00:17.0 AHCI | 00:17.0 | 4 | 根 SSD 和数据盘共享控制器 | 已验证；不能按磁盘只改 PCI 归属，需块设备后端 |
| 00:1f.6 I219-V | 00:19.0 | 11 | PPPoE 配置、远程管理链路 | 已验证；转移前必须先建立替代管理网络 |
| 01:00.0 GP102 | 00:1a.0 | 1 | 00:01.0 桥、01:00.1、显示输出 | 已验证 Wayland；与 HDMI 一起作为分配单元 |
| 01:00.1 HDMI | 00:1a.1 | 1 | GPU 电源/复位域，snd_hda_intel | MSI 自动加载已验证，未验证实际出声 |
| 05:00.0 AX210 | 计划 00:1b.0 | 13 | 00:1c.7 桥（组8）、iwlwifi 固件 | 待接入；Wi-Fi 与 USB 蓝牙不是同一 PCI 功能 |
| 04:00.0 ASMedia xHCI | 计划 00:1c.0 | 12 | 00:1c.4 桥（组7）、其全部 USB 端口 | 待接入；适合后续独占 USB Zone 候选 |
| 00:1f.3 PCH HD Audio | 计划 00:1d.0 | 10 | LPC/PMC/SMBus 同组、codec、模拟插孔 | 待接入；不可宣称能单独安全拆 Zone |
| 00:1f.4 SMBus | 未分配 | 10 | 主板管理总线、LPC/PMC | 待评估；SPD/传感器枚举不授权任意总线写操作 |
| 00:16.0 MEI | 未分配 | 3 | Intel ME/固件管理 | 待评估；优先留管理域，不因有驱动就开放 |
| 00:1f.2 PMC | 未分配 | 10 | 平台电源管理 | 管理域保留候选；不向普通客体开放整个平台复位 |
| 00:1f.0 LPC/eSPI | 未分配 | 10 | legacy I/O、固件、看门狗等 | 管理域保留候选；需端口级隔离 |

PCIe 桥目前保持固件设置，对客体隐藏，不能让客体任意修改 secondary
bus、桥窗口或复位下游。桥节点：00:01.0（组1，GPU）；00:1c.4（组7，
ASMedia）；00:1c.7（组8，Wi-Fi）；00:1b.0（组5）、00:1c.0（组6）、
00:1d.0（组9）在采集时下游为空。计划客体地址不是物理桥地址，二者不能混用。

## BAR / IRQ 合同

### x86 legacy INTx 路由实现（2026-09-14）

Zone 配置中的 `interrupts` 现在会初始化 Zone 的 IRQ bitmap。非 Zone0
只有在 bitmap 明确声明某个 GSI 时，客体写入虚拟 IOAPIC RTE 才会同步
到物理 IOAPIC；其它 GSI 仍保持虚拟-only。这样避免按 BDF 猜测共享
线路归属。核显 00:02.0 的 ACPI `_PRT` 为 INT-A→GSI16，因此 IGD
候选显式声明 `[16]`；Zone0 现有 IRQ16 使用快照为空，仍需实机验证。

这不是 MSI/MSI-X 代理：MSI 仍需单独的物理中断重映射/目标校验。不要
把新的 GSI 直接加入 Zone1，也不要在未检查 `/proc/interrupts`、ACPI
`_PRT` 和 Zone0 owner 前复用共享线路。

| 节点 | 物理 MMIO 起点/大小 | 中断要求 |
|---|---|---|
| PCH xHCI | df330000/10000 | 已验证；切换 VT-d 前停止控制器 |
| AHCI | df348000/2000、df34b000/1000、df34c000/1000 | 已验证 MSI；根盘连续可访问 |
| I219-V | df300000/20000 | 已验证 MSI；保留 PPP 管理链路 |
| GP102 | de000000/1000000、c0000000/10000000、d0000000/2000000；I/O e000/80 | GPU 与音频联合评估复位 |
| HDMI | df080000/4000 | 原生 pin C→GSI17；Zone0 强制 MSI，关闭控制器省电复位 |
| AX210 | df100000/4000 | MSI-X 表位于 BAR0+2000；需验证固件与中断 |
| ASMedia | df200000/8000 | MSI-X；停止固件 DMA 后交接 |
| PCH Audio | df340000/4000、df320000/10000 | 与 HDMI 同驱动，参数不可无条件全局套用 |
| SMBus | df34a000/100，I/O f000/20 | 同组共享平台资源 |
| MEI | df34d000/1000 | 平台管理，另行评估 |

所有 BAR 都须同时核对板级 EPT、客体 ACPI _CRS、PCI 配置空间和
VT-d context。VT-d requester 使用**物理 BDF**；客体地址只是枚举视图。
改变 BIOS/硬件后重新采集，不把本表当永久硬件常量。

## 每个设备的接入流程

1. 清点真实资源和组依赖；选择未占用客体 BDF，写清归属。
2. 单独新增 EPT/ACPI/PCI 条目；不改变其他设备、桥编号和默认 Arch。
3. 交接前停止旧 DMA、屏蔽中断、验证 BME；设备驱动重新建立 DMA 队列。
4. 构建与重叠/ACPI测试；保存旧 ELF 哈希和整套备份。
5. 实机重启，检查实际地址与 boot ID；确认根盘、8CPU、内存、键鼠、
   PPP、GPU/桌面及失败服务。只枚举成功不算设备可用。
6. 验证新设备驱动、中断和实际功能；不足的部分明确标记未验证。
7. 独立提交并更新本表；失败先隔离/回退本项，不叠加下一项。

顺序：HDMI（已完成驱动/启动验证）→ AX210 → ASMedia → PCH Audio。
其余平台控制器先评估用途与隔离，不以“清单全勾选”为目标。

## 后续拆 Zone 的必要条件

- 每个物理设备只能有一个 owner，DMA 映射只含该 Zone 的内存。
- MSI/MSI-X/INTx 目标必须限制在 owner CPU 集合，不能保留直写物理
  MSI 表或 unrestricted I/O 的单 Zone 假设；核查中断重映射能力。
- 配置空间写保护、跨组/桥复位范围、P2P/ACS、DMAR/RMRR 都要验证。
- 转移时先在旧驱动中解绑/停止，阻断 DMA/IRQ，清理并失效 VT-d
  context/IOTLB，再建立新 owner；不在线仅改 BDF 或掩码。
- GPU/HDMI 作为共享复位单元；组10保留管理域；AHCI根盘不能直接
  转走；Wi-Fi 与蓝牙拆分需单独处理 USB 所有权。
- 当前 PIO 根域基本直通及有限 APIC/MMIO 仿真不构成不可信 Zone
  的完整安全边界。多 Zone 测试前必须收紧这些权限并验证故障隔离。
