# Z270：从手写 BDF 到依赖驱动的设备分配

配套图：[ARCHITECTURE.svg](ARCHITECTURE.svg)。中文 SVG 为代码绘制，
可直接在浏览器或 Inkscape 中查看和修改；不需要外链字体或图片。
主图采用分层拓扑与设备图标，便于讨论资源归属；原详细文字图保留为
[ARCHITECTURE-detailed.svg](ARCHITECTURE-detailed.svg)。

## 本次实际实现

`board.rs::zone0_native_inventory!` 的每行包含稳定名称、物理
bus/device/function、原生 IOMMU 组、上游节点名称。清单一次声明，
同时展开为审核元数据 `Z270_PCI_INVENTORY` 和现有 ABI 所需的
`ROOT_PCI_DEVS` 数组；长度自动计算，客体 BDF 自动等于物理 BDF。
域号固定为此实机的 segment 0。这是编译期板级分配整理，**不是新增
运行时自动发现、热插拔、设备迁移或通用资源分配器**。

现有发现链不重写：`hvisor_pci_init` 枚举物理设备进入
`GLOBAL_PCIE_LIST`，`Zone::guest_pci_init` 消费分配表，建立 vPCI
视图与 VT-d 关联；桥配置会经过虚拟配置处理。BAR 的 EPT 映射和
ACPI 资源仍显式维护，不能仅加一行清单就保证新硬件可用。

静态测试校验 19 个预期 BDF、唯一性、父节点存在、GPU/HDMI 共同
依赖、组10关系、原地址策略、MMIO/RAM 重叠及关键 ACPI 窗口。
当前不提供看似可用的 `owner=zone1` 开关，因为其它四类资源尚未
随着它同步更新。原生组号是历史观测，不是硬件不变的常量或安全证明。

## 建议下一阶段接口（未实现）

1. **发现快照**：记录 segment/BDF、vendor/device、class、桥父子树、
   BAR 类型及大小、MSI/MSI-X、DMAR/RMRR、ACS 与复位范围。
   与已核准板级快照比对；缺失、地址漂移或新增设备应明确拒绝或报告，
   不自动把未知设备及任意 MMIO 暴露给客体。
2. **分配单元**：按共享 DMA/复位/平台依赖形成具名集合，例如
   `display={gpu,hdmi}`。上游桥的控制权与端点使用权分别建模；
   不简单按 IOMMU 组号合并整棵树，也不忽略桥复位影响。
3. **策略编译**：`assign(display, zone1)` 输出一份统一计划，涵盖
   vPCI、EPT、VT-d、IRQ/CPU 和 ACPI；检查唯一 owner、内存边界、
   中断目标、资源冲突、保留管理链路及跨域共享约束。
4. **交接事务**：解绑并停止旧驱动/DMA/IRQ → 撤销旧映射并失效
   IOTLB → 可验证的设备复位 → 新映射/新驱动。失败保持设备停用，
   不在新旧 owner 中同时开放；必须规定安全回退而非仅恢复数组。

## 分离优先级

- GPU + HDMI：共同评估复位；首次转移会影响当前物理桌面。
- ASMedia xHCI：相对独立候选，但必须实际测试外设传输与复位。
- AX210：PCI Wi-Fi 与经 PCH USB 连接的蓝牙分别建模。
- AHCI：根 SSD 与数据盘共控制器，不能把 PCI 功能拆成两块盘；
  分盘共享需块设备后端等额外实现。
- I219-V：承担远程管理，转移前建立替代链路。
- LPC/PMC/PCH HDA/SMBus：原生组10，平台依赖强，优先留管理域。

## 验证边界

此前运行基线接入 11 个 PCI 功能，并验证桌面/键鼠/根盘/管理网络。
本地全拓扑候选有 19 个功能、8 线程、全部核准的可用 RAM；增加了
平台/桥可见性。尚未部署重启验证，不声称新增平台控制、全部中断路由、
跨 Zone DMA 隔离或任意 PCI 热插拔已经可用。

维护源文件、图与说明均在本目录。验证输出统一放在维护目录
`evidence/baremetal/`；未修改 QEMU 配置中的既有用户变更。
