# boardgen：Intel 单可信 Zone0 配置候选生成器

## 新增：迁移检查、平台包与验收

```bash
# 独立检查快照；缺失证据会标 unknown，不表示可启动
python3 tools/boardgen/boardgen.py check --snapshot hardware.json --output capabilities.json

# 比较旧机器/旧BIOS与新快照，包括 BAR、组、父桥、CPU、RAM、ACPI
python3 tools/boardgen/boardgen.py diff \
  --before old-hardware.json --after hardware.json --output migration.json

# 生成完整的平台源目录包（仍需要审核策略，不包含 kernel/initrd）
python3 tools/boardgen/boardgen.py generate \
  --snapshot hardware.json --policy policy.json \
  --board platform/x86_64/z270/board.rs \
  --acpi platform/x86_64/z270/minimal-dsdt.asl \
  --platform-template platform/x86_64/z270 --name my-intel-board \
  --output generated-package
```

`--platform-template` 会额外打包 `platform/x86_64/my-intel-board/` 下的
board、DSDT、linker、Kconfig、Cargo模板、platform.mk和显式配置标记。
配套 `build.rs` 通过 `boardgen-profile` 识别已审核的 minimal-ACPI
适配族，不再要求新平台名称必须叫 z270；原 z270 路径继续兼容。
模板不因打包自动适配新CPU/芯片组，需核对linker CPU_NUM等。

所有生成模式还输出：

- `capabilities.json`：能力检查与未知项目，不承诺可启动。
- `memory-layout.json`：实际候选映射、启动常量、hvisor保留区以及尚未
  分配的原生 RAM 页区间。剩余RAM只作为建议，**不自动重定位启动区**。
- `acceptance.py`：在对应身份映射的 Zone0 中运行的只读检查。
  对比CPU ID、PCI设备ID和Zone0标记，采集驱动/MSI、服务、挂载、网络、
  内核警告与时钟源；基本检查失败返回非零。实际设备功能仍需人工或专项测试。
- `manifest.json`：所有输出文件的 SHA-256（不含清单自身）。
- 平台包另外有 `BUILD.md` 与构建模板哈希清单。

```bash
# 在已经部署并启动的目标 Zone0 执行；本工具不会自动部署
python3 acceptance.py > acceptance.json
```

验收报告可能包含本机IP、挂载路径及日志，公开前请检查。
验收脚本不执行磁盘写入测试、服务启停、设备复位、外部网络连接或重启。

当前验证：27项自动化测试通过，包含CLI打包/哈希、迁移差异、内存
区间减法与Rust语法/接口检查；原Z270完整release构建通过。
新主板与新名称平台包尚未实机启动验收；测试快照是合成数据。

第一版是**审核模板驱动**，不是任意同代主板即插即用。Python 3.11+
标准库，无 pip 依赖。所有源文件集中在本目录；不修改运行机器的驱动、
PCI 配置、BIOS、GRUB，不自动编译安装或重启。

## 工作流

先在目标机器的原生 Linux（不是 hvisor Zone0）启动，启用 VT-x/VT-d，
并让 Linux 建立 IOMMU 组。用 root 只读采集；输出文件不能已存在：

```bash
sudo python3 boardgen.py collect --output hardware.json
```

采集 sysfs PCI 资源元数据、父子拓扑、IOMMU 组、驱动、CPU/APIC、
System RAM、DMI 型号/BIOS 版本、六张原始 ACPI 表。不会读取 PCI config
或 mmap BAR，不扫描 SMBus，不收集密码、网卡配置及机器序列号。
快照中的 DSDT 属于机器固件信息，公开前仍应自行检查。

在仓库根目录创建待审核策略：

```bash
python3 tools/boardgen/boardgen.py policy \
  --snapshot hardware.json \
  --board platform/x86_64/z270/board.rs \
  --acpi platform/x86_64/z270/minimal-dsdt.asl \
  --output policy.json
```

策略默认 `reviewed=false`、hvisor 保留范围无效，因此不能直接生成。
审核人需要：

1. 为目标硬件准备/调整板级模板的 RAM、启动映射、CPU数、ECAM及总线范围。
   第一版只识别当前 Z270 的 `zone0_native_inventory!` 模板接口。
2. 从实际 ELF 的 `skernel`/`__hv_end` 确定物理保留区，填入
   `hypervisor_reserved=[start,end]`；末端不包含。不要照抄别的编译结果。
3. 审核 root UUID、内核与 initrd 地址/大小、framebuffer、固件内存、
   ACPI 中断路由、桥窗口/复位和设备 quirk。调整模板后重新创建策略。
4. 需要新增 BAR 页时，核准后填入 `approved_extra_bar_pages`。
   自动生成仅支持添加整页、无部分重叠的 MMIO；I/O port 与 ROM 不自动映射。
   页取整可能覆盖相邻设备资源，必须审核共享页。
5. 核对策略中所有提示后将 `reviewed` 改为 true。它是审核确认，
   **不是工具已经证明安全的标志**。

```bash
python3 tools/boardgen/boardgen.py generate \
  --snapshot hardware.json --policy policy.json \
  --board platform/x86_64/z270/board.rs \
  --acpi platform/x86_64/z270/minimal-dsdt.asl \
  --output generated-board
```

输出为 `board.rs`、`minimal-dsdt.asl`、`hardware.json`、`policy.json`、
`allocation.svg`、`report.json`。目录必须不存在；不覆盖现有平台。
PCI 表按快照生成原生 BDF 恒等分配；自动追加明确审核的新 BAR 页，
保持模板 RAM 和启动区域索引。架构 SVG 同样来自快照。

**ACPI 文件目前按审核模板原样输出，不自动翻译固件 AML 或推断 _PRT。**
因此新增 BAR/总线前必须同时更新模板 ACPI。这一限制在报告中显式记录。
默认输出不含平台构建目录，添加 `--platform-template` 可生成配套
平台源目录。两种模式仍需仓库公共代码、字体、kernel/initrd 配套，
不能直接用单文件替代任何机器的启动包。

## 拒绝条件和边界

- 虚拟化环境、非 Intel VMX、隐藏/缺失 RAM 地址、缺失 IOMMU 组。
- 快照、模板或 DMI/BIOS 变更；要求重新审核。
- 多 segment、非零起始bus、稀疏CPU/APIC、超过8线程。
- 模板 RAM 在当前原生 RAM 外、EPT冲突、hvisor/APIC/VT-d/ECAM暴露。
- 未批准 BAR、部分重叠BAR、缺父节点、重复BDF、超ABI容量。
- DMAR RMRR：v1 明确拒绝，须新增专项平台适配后支持，不能静默忽略。

当前检测不能证明冷启动后的固件地址稳定，也不证明复位/IRQ隔离。
Linux 内存快照并不等于完整固件 e820；v1 只用于校验审核后的 RAM
布局，不自动把所有地址空洞都分配给 Zone0。同代机器也可能需要修改
公共 hvisor 的 APIC、ACPI、设备交接代码，生成器不能替代它们。

## 验证

```bash
python3 -m unittest discover -s tools/boardgen -p 'test_*.py' -v
```

测试数据是**合成输入**，不冒充原生实机采集或启动验证。
部署前还必须编译，使用 `baremetal/verify-layout.py` 检查最终 ELF，
执行 ACPI 求值、设备依赖和启动镜像校验，保留整套回退，随后实机验收。
本工具不执行这些破坏性/中断性步骤。
