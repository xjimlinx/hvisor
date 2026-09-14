# 双 Linux / HD630 适配进度（2026-09-14）

分支：feat/z270-dual-linux-igpu。尚未部署，不是可启动发布。

## 已实现与验证

- DMAR解析保留多个DRHD，按显式endpoint scope优先、include-all兜底。
  真机00:02.0路由fed90000，AHCI/USB/NVIDIA/AX210路由fed91000。
- 明确拒绝坏长度/校验和、重复scope或include-all、多segment、多跳PCI
  path及尚未实现的sub-hierarchy scope，不再静默选择最后一个DRHD。
- 每单元独立root/context/QI表；填表、失效及故障读取覆盖全部单元。
- context中的domain ID来自显式目标Zone，不再读取调用CPU所属Zone。
- 首次全局固件DMA交接使用所有单元的完整设备归属集合；以后activate
  不重复停设备，防止Zone1启动再次停掉Zone0硬件。
- 相同BDF不能静默覆盖为另一Zone；动态设备转移尚未实现。
- 6项Rust回归测试通过，包含读取新原生硬件快照DMAR的测试；Z270
  release构建通过。此时board.rs仍是旧单Zone布局，禁止部署该ELF。

## 已确认硬件

原生Linux中HD630 [8086:5912]由i915驱动，主板HDMI connected，
模式含1366x768；NVIDIA HDMI同时connected，模式含1920x1080。
CPU按MADT顺序0/1/4/5与2/3/6/7分为两个完整物理核心的集合。
核显专用RMRR为[0x7b800000,0x80000000)，不能当普通RAM分配。
核显BAR为dd000000/16MiB、b0000000/256MiB，IO f000/64B。
新原生内存布局改变，旧单Zone RAM映射已不适用。

## 部署前仍需完成

1. 新内存布局和4+4 CPU分配；Zone1暂定10GiB，其余核准RAM留Zone0。
2. Zone1安全加载路径：现有hvisor-tool通过Zone0的memremap写镜像，
   并不是hvisor侧受限加载hypercall。如果直接取消Zone0对Zone1内存
   的EPT访问，加载将失败；若永久共享又会让Zone0 DMA获得访问。
   需要选择并实现受限加载/暂存复制机制，不能只改两个内存数组。
3. 非根Zone ACPI目前过滤FADT/SSDT，最小DSDT仅替换Zone0，需建立
   Zone1独立CPU/PCI/核显固件视图与启动控制台。
4. 核显RMRR/stolen/OpRegion/VBT、交接与复位；目前解析DRHD不等于
   已实现这些要求。不能把本次构建通过当作i915直通可用。
5. 四线程initramfs Linux启动、时钟/中断测试，之后才接核显与桌面。

所有静态生成物/实机快照在维护目录evidence/baremetal，未写入GRUB，
未换内核、NVIDIA驱动，未停止原生Arch或修改在用磁盘。
