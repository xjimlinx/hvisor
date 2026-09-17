# Windows / UEFI bring-up：第一阶段

分支：`feat/z270-windows-uefi-bringup`，基线 `911066f`。
目标仍是 Z270 实机上的 Linux Zone0 + Windows Zone1，不是 QEMU 虚拟机。
当前状态：**2026-09-17 Z270 实机复位探针已输出 HVFW0，Zone0 SSH 保持可用；尚未进入 UEFI/Windows。**
不替换已验证的双 Linux 启动项、后端或系统盘。

## 本轮实现

- 为 Z270 非根 Zone 增加显式实验启动模式 2，保留 Linux=0、Multiboot2=1
  和既有 16 字节启动模式结构。仅 Zone1、无附加信息地址时允许设置。
- BSP 按固件复位入口 CS.base=0xffff0000、CS=0xf000、IP=0xfff0 启动。
  CR0/CR3/CR4、栈和通用寄存器另行初始化，不携带 Linux 参数；
  VMX 所需的宿主 VMXE 仍由掩码隐藏。AP 的现有 SIPI 路径不改。
- 固件模式不调用 Linux BootParams::fill，不拷贝实机 ACPI 覆盖固件 RAM。
- 首轮只接受现有 Zone1 物理池中的、页对齐且 HPA/GPA 均不重叠的 RAM。
  必须覆盖低端 RAM 和复位向量；禁止 PCI、MMIO、virtio、IVC、物理 IRQ。
- 未知 WRMSR 从宿主 unwrap panic 改为客体 #GP(0)，不推进故障 RIP。
  这不是完整 MSR 兼容层；已知但未实现的 MSR 和 RDMSR 仍待逐项审查。
- 配套 hvisor-tool 在任何镜像复制前拒绝未知 boot_protocol。
  **它目前刻意拒绝 firmware-reset，避免旧工具误按 Linux 载入 ROM。**

## 最小复位探针

```sh
python3 platform/x86_64/z270/prepare-firmware-probe.py /一个新的输出目录
```

生成 64 KiB 的 probe.rom，预期仅输出 `HVFW0` 后 HLT，不访问 PCI/磁盘。
probe-plan.json 标注 deployable=false；它不是可以直接启动的 Zone JSON。
ROM 映射 GPA 0xffff0000 → HPA 0x5f1000000，**绝不能写物理主板 flash**。
这只是从 0xfffffff0 取指的探针，不是 UEFI Shell。

2026-09-16 本地结果：

- `CARGO_BUILD_JOBS=32 make ARCH=x86_64 BOARD=z270 LOG=info elf` 通过，有既有警告。
- firmware.rs 三个宿主单元测试通过（地址、合法布局、危险布局拒绝）。
- Z270 设备契约测试通过；配套工具协议解析七例测试和用户态链接通过。
- 探针长度/跳转目标验证通过，SHA256：
  `dad792ffd790a8eabce78cda9a83756bbbd5a8dae2b13720e65215a2ca9f4890`。
- 没有执行 QEMU 验证，也不以 QEMU 行为替代实机依据。

## 下一步按顺序完成

1. [已实现，见下方进展] 为 staging 增加明确的 firmware 能力协商；旧 hvisor 对模式 2 可能按
   Multiboot2 解释，不能只依赖相同的旧 staging MAGIC。
2. [已实现固定探针 loader，见下方] 实现有界的固件 loader 和清零：确认 Zone1 不在运行，预检全部映射，
   只写保留物理池、加载完整 ROM、设置模式后启动，不热停当前磁盘后端。
3. 实机观测 HVFW0，并检查 Zone0 仍可远程访问；失败保留串口日志。
4. 对 EDK II 制定 hvisor 平台实现，再启动 UEFI Shell。
   当前 OVMF 不是任意硬件的通用 BIOS，其平台初始化依赖虚拟芯片组、
   fw_cfg、内存发现及固件存储接口，不能把 stock OVMF 文件直接当成完成适配。
5. WinPE → virtio-pci 磁盘/网络 → 独立测试盘 → Windows 图形/USB；
   Windows 11 再处理 TPM/Secure Boot/CPU 要求。保留原 Arch 磁盘。

参考：
- https://github.com/tianocore/edk2/blob/master/OvmfPkg/README
- https://github.com/tianocore/edk2/blob/master/OvmfPkg/Library/PlatformInitLib/Platform.c
- https://github.com/tianocore/edk2/blob/master/OvmfPkg/RUNTIME_CONFIG.md
- Intel SDM 的处理器复位状态、VM-entry guest-state 和 RDMSR/WRMSR 章节。

## 第二批进展：能力协商与固定探针 loader

现有 hypercall 12 增加只读查询 `(arg0=0x46575031,arg1=0)`，仅在尚未
封存且 Zone1 不存在时返回 `0x5a314602`。旧版本对非零 arg0 返回错误，
因此不会误把模式 2 当 Multiboot2。普通 `(0,0)` staging 查询保持不变。

配套驱动新增 `HVISOR_Z270_FIRMWARE_CAP`（ioctl 37），用户态独立程序为
`hvisor-tool/extras/z270-firmware-probe.c`。只有明确指定 `--start-probe` 才
可能写客体 RAM；`--dry-run` 不打开 /dev/hvisor。它只接受逐字节匹配已审查
HVFW0 探针的 64 KiB 文件，未开放任意 OVMF 加载。固定映射仅低端 16 MiB
和 64 KiB ROM，清零已映射低端 RAM；检查 ABI、能力、仅有 Zone0，全部
复制经过既有 one-shot staging，每页仍受 SEALED/Zone1 检查。

本轮 hypervisor 编译、布局测试、设备契约测试再次通过；后端测试覆盖
65,536 个 ROM 单字节破坏、旧 capability、ENOTTY、EBUSY、非法上传边界。
驱动使用本地 **7.2.6-arch2-1** headers 完成编译检查；这不是目标机
6.18.50-2-lts 模块，禁止直接部署本地 driver/hvisor.ko。
实机 IPv6 SSH 再次超时，尚未安装候选或验证 HVFW0。

下一实机步骤仍需：核实最新地址与内核 → 单独编译对应 headers 的驱动 →
保留回退、安排不自动启动现有 Zone1 的一次性实验启动 → 只测试 HVFW0。
不能在磁盘后端占用中的当前 Zone1 上热替换固件或强行卸载 hvisor.ko。

## 2026-09-17 实机连接与候选准备

- 用户提供的 `10.71.150.56` 已通过既有 SSH 主机密钥验证连接；为 ppp0 地址。
- 当前是原生 Arch，`6.18.50-2-lts`，cmdline 无 `hvisor.zone0=1`；
  hvisor-zone0、hvisor-zone1、hvisor-virtio-test 均 inactive。
- 在 `/root/arch-z270-maintenance/hvisor/candidates/windows-uefi-probe-20260917/`
  集中准备候选；只传输驱动/头文件/探针 loader 源码，在目标机用匹配的
  6.18.50-2-lts headers、8 线程编译成功，模块 vermagic 匹配。
- 未替换运行时模块、未加载模块、未改 GRUB、未重启；仍未验证固件实际执行。
- 实验启动前必须单独屏蔽 Zone1、virtio 后端及旧 Zone0 模块自动加载。
  客体 cmdline 内嵌于 board.rs，不能误以为给 GRUB 菜单附加参数即可生效。
  应生成独立候选启动项，保留原生 Arch 默认项和现有双 Linux 项。

## 2026-09-17 RAM-only 复位探针实机通过

构建：`Z270_FIRMWARE_PROBE_BOOT=1 CARGO_BUILD_JOBS=32 make ARCH=x86_64 BOARD=z270 LOG=info elf`。
该显式构建开关只追加本次 Zone0 cmdline 的三个 `systemd.mask=` 和
`hvisor.firmware_probe=1`，不修改永久服务配置；未设置开关的普通构建不追加。

使用本目录 `firmware-probe-grub.cfg` / `install-firmware-probe.sh` 新增
`hvisor-z270-firmware-probe` 项。候选 ELF SHA256：
`c5fe4a02cb4176b3d125178108aa1b714de38c1ee1b8ee490cbe0f68c7fb4a1d`。
通过 `grub-reboot` 一次性启动；原生 Arch 仍为默认项，原有 payload 哈希均未改变。

实测：三个服务为 masked-runtime；加载匹配 6.18.50-2-lts 的候选模块；
固定 loader 启动成功，客体 UART 精确输出 `HVFW0\r\n`。
宿主日志确认 CPU2 从 `0xfffffff0` 启动 Zone1，EPT 仅低端 16 MiB 和高端
64 KiB 两段，PCI devices=0/buses=0。之后仍能通过 SSH 执行命令。
这证明复位取指和该串口探针可执行，不证明 UEFI、AP 启动或 Windows 兼容性。

发现一条 `VT-d FRCD[0]: sid=00:02.0, reason=0x01, addr=0x0`。
本次无核显分配，不能归因为 Windows 或宣称无硬件错误；后续隔离核显固件
残余 DMA 状态时需核对，不能放开所有 DMA 来掩盖。没有为此改动设备策略。

完整 UART/宿主日志集中保留于目标候选目录以及本地
`hvisor/artifacts/windows-uefi-probe-20260916-v2/probe-{uart,hvisor}.log`。
验证后安排一次性返回原有 `hvisor-z270-experimental` 双 Linux 项，
不让探针长期占据 Zone1。
