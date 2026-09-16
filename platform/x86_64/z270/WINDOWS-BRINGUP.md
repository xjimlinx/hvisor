# Windows / UEFI bring-up：第一阶段

分支：`feat/z270-windows-uefi-bringup`，基线 `911066f`。
目标仍是 Z270 实机上的 Linux Zone0 + Windows Zone1，不是 QEMU 虚拟机。
当前状态：**复位入口代码已编译，尚未实机验证，更没有进入 UEFI/Windows。**
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

1. 为 staging 增加明确的 firmware 能力协商；旧 hvisor 对模式 2 可能按
   Multiboot2 解释，不能只依赖相同的旧 staging MAGIC。
2. 实现有界的固件 loader 和清零：确认 Zone1 不在运行，预检全部映射，
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
