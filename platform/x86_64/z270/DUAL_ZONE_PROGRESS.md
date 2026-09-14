# 双 Linux / HD630 适配进度（2026-09-14）

分支：feat/z270-dual-linux-igpu。Zone0 + Zone1 RAM Linux已在实机同时运行；
Zone1尚无核显、USB、桌面或交互shell，不是完整双桌面发布。

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
  release构建通过。421d168阶段board.rs仍是旧布局；下面第一阶段已修正并测试。

## 已确认硬件

原生Linux中HD630 [8086:5912]由i915驱动，主板HDMI connected，
模式含1366x768；NVIDIA HDMI同时connected，模式含1920x1080。
CPU按MADT顺序0/1/4/5与2/3/6/7分为两个完整物理核心的集合。
核显专用RMRR为[0x7b800000,0x80000000)，不能当普通RAM分配。
核显BAR为dd000000/16MiB、b0000000/256MiB，IO f000/64B。
新原生内存布局改变，旧单Zone RAM映射已不适用。

## 部署前仍需完成

1. 新内存布局已按原生快照修正：Zone0 CPU mask=0x33，RAM
   23398719488 字节；Zone1 CPU mask=0xcc 尚未启动，预留
   HPA [0x5f0000000,0x870000000) 共10GiB，Zone0 EPT不映射。
2. Zone1安全加载路径：现有hvisor-tool通过Zone0的memremap写镜像，
   并不是hvisor侧受限加载hypercall。如果直接取消Zone0对Zone1内存
   的EPT访问，加载将失败；若永久共享又会让Zone0 DMA获得访问。
   需要选择并实现受限加载/暂存复制机制，不能只改两个内存数组。
3. 非根Zone ACPI目前过滤FADT/SSDT，最小DSDT仅替换Zone0，需建立
   Zone1独立CPU/PCI/核显固件视图与启动控制台。
4. 核显RMRR/stolen/OpRegion/VBT、交接与复位；目前解析DRHD不等于
   已实现这些要求。不能把本次构建通过当作i915直通可用。
5. 四线程initramfs Linux启动、时钟/中断测试，之后才接核显与桌面。

上面2/5的最小加载与启动部分已由下述实机实验完成；完整生命周期、
设备隔离、ACPI和USB/核显仍待实现。不要把旧待办当作当前启动状态。

所有静态生成物/实机快照在维护目录evidence/baremetal。

## 2026-09-14 第一阶段部署测试

- 原生 boot_id：41f4a2fe-9928-407b-b93a-d2a85dfe2ea7，远程核对
  /proc/iomem 与核显启用后的快照一致。
- 新 ELF SHA256：731046b56f2b4ad9f95bee5ade0729ce4d84dffd8b11044c5b3afa1213b78efc。
- hvisor占用 [0x200000,0x47cc000)，与Zone0所有32个静态映射无重叠；
  native RAM包含检查、Zone1预留排除、GPA重叠检查、initrd落点检查通过。
- DMAR 6项、PCI枚举2项、设备契约检查通过；release构建成功（39个既有警告）。
- 仅测试Zone0缩至四线程和新RAM布局、多DRHD初始化；不宣称Zone1/i915可用。
- 使用维护目录install-audit-candidate.sh备份后替换实验ELF，保留原生Arch
  默认项，grub-reboot只选择一次实验项。内核与NVIDIA驱动不变。
- 实机启动成功，boot_id=5aed664e-11f0-467f-ac56-6a9c01d5a0ab。
  nproc=4，lscpu为两个物理核心各两个线程，MemTotal约21796MiB。
- Wayland plasmalogin greeter active；NVIDIA GTX1080Ti 580.178.04
  nvidia-smi成功，40°C。systemctl --failed为空；httpd/jellyfin active。
- /dev/sdb2挂载/，/dev/sda1挂载/srv/storage，均ext4。
- PPP和Wi-Fi IPv6均SSH回连；PPP地址未变，IPv4 Wi-Fi从本控制机不可达。
- USB鼠标/触屏/蓝牙已枚举；本轮未人工验收键盘输入。
- guest kernel日志没有匹配到soft lockup、DMAR fault、I/O error、
  xHCI error或NVIDIA Xid。仍有hdaudioC0D2无法配置告警，未做音频验收。
- grubenv next_entry已消耗为空，默认仍原生Arch。旧ELF备份目录：
  /root/arch-z270-maintenance/backups/hvisor-audit-20260913-QEmeV5。
- 这是短时启动/服务检查，不是长期稳定性或双Zone隔离验收。

## Zone1 initramfs 实机成功（2026-09-14）

- 维护目录baremetal/zone1-minimal包含静态PID1、镜像生成、配置生成、
  日志读取和测试，以及后续USB控制器依赖说明。
- 新增z270专用受限4096B加载HC12/13、Zone1日志HC14、hvisor日志HC15；
  配套驱动从Zone0暂存页复制，不向Zone0 EPT暴露Zone1 RAM。
- Zone1 Linux6.18.50-2-lts，4 CPUs，/init pid1，proc/sys挂载成功，
  周期约5s心跳；报告RAM10420076544字节（分配10GiB扣除内核等保留）。
- 最小initramfs335819字节，SHA256
  1b231dd745c2bbd8fc7ebb9f80dbab6e2522aae8f4767a862180228d37a574d1。
- 修复非物理CPU0担任Zone1 BSP时APICBASE缺BSP位；否则Linux误认为
  crash kernel并限制为1 CPU。正常8250 tty IRQ路径仍有同步卡顿，
  最小镜像通过earlycon和/dev/kmsg绕开，不宣称串口IRQ已修复。
- 用户报告Zone0花屏：Zone1 start后约10ms出现KWin Xid31 MMU Fault。
  发现hvisor继续写NVIDIA已接管的旧固件framebuffer，改为Zone0首次
  VM进入前停止该显存写入。INFO不关闭，保存到独立缓冲和串口。
- 当前实测ELF ec449e26e9592cce143a2ed0237854c6784da26cf4e767f93602d44658a27f40，
  boot_id e1ce6cfe-f62b-4981-94ce-8563dddf77a9。修复后短时双Zone复测
  KWin继续运行，未见Xid；长期偶发花屏还需观察。
- Zone1不设置开机自启；配套模块目前手动加载，原生Arch仍默认启动项。
- 证据在evidence/baremetal/zone1-ramfs-boot-20260914.log及远程candidate
  的boot-fb-handoff.log/hvisor-fb-handoff.log。部分旧CPU日志写死
  “Zone0”，应以zone list和Zone1 INIT_READY为准。

## Zone1 核显首轮适配（2026-09-14，实机）

- 原生快照确认 00:02.0 是 HD630/Kaby Lake，BAR0=`dd000000/16MiB`、
  BAR2=`b0000000/256MiB`、BAR4=`f000/64B`。最小 DSDT 已发布这些固定
  资源；此前 i915 把 BAR 搬到 `0xfef9xxxx` 的 EPT/MMIO fault 不再出现。
- i915 还要求通过 Intel ISA bridge 识别 Sunrise Point PCH。00:1f.0 属于
  Zone0，不能直通给 Zone1，因此新增虚拟 `00:1e.0` PCH identity stub
  （8086:a2c5、class 0601，无 BAR/MSI/DMA）。真实日志确认 stub 已插入，
  i915 不再在 `ilk_hpd_irq_setup` 触发 NULL dereference。
- 最近一次未重启的实机状态：Zone0（CPU 0,1,4,5）和 Zone1（2,3,6,7）
  同时 running；Zone1 i915 完成 DMC、注册 3 个 plane，并建立 `fb0`，
  心跳持续，hvisor 无 EPT/MMIO panic。
- 当前明确剩余缺口：i915 报 `can't find IRQ for PCI INT A`，因为 GSI16
  与Zone0平台路由共享，不能直接把物理GSI16再分配给Zone1；显示引擎
  仍反复报告 `PLANE:33 ... SURF=0xc0000`，物理 HDMI 连接/亮屏尚未验收。
  下一步应做显示引擎安全交接和HPD中断代理，不能靠继续扩大RAM或PCI
  BAR映射解决。EDID 固件覆盖和1366x768强制模式已加入候选配置，但没有
  伪造“已亮屏”结论。

- 针对 `SURF=0xc0000` 的下一项低风险修正已准备：i915 源码显示 Kaby Lake
  默认开启 fastboot，`i915.fastboot=0` 会强制初始 modeset、清理继承的
  BIOS plane 状态。该参数已写入远程候选 `zone1-fastboot-off.json`，当前
  正在运行的 Zone1 未被重启或替换；验证仍需一次受控重启并观察实体 HDMI。
