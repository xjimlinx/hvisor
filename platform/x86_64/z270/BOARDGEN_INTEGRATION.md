# boardgen 集成验收（2026-09-13）

独立仓库：https://github.com/xjimlinx/boardgen （私有）。

build.rs支持`platform/x86_64/<name>/boardgen-profile`内容为
`intel-trusted-zone0-v1`，编译该目录的minimal-dsdt.asl，复用现有
z270_minimal_acpi配置族。原z270无标记的构建路径保持兼容。
标记不表示任意Intel主板已适配；仍要求审核模板和硬件资源。

## 已完成

- boardgen参考模板同步自adcdd1e：保留最新桥中断路由，移除无效
  earlycon=efifb，根UUID保持全零占位符。
- 独立仓库30项测试通过。
- 合成快照生成`boardgen-smoke`平台，在独立本地克隆中应用构建补丁，
  release构建成功；没有启动QEMU，也没有部署实机。
- 严格检查ELF范围[0x200000,0x47c8000)、30个物理映射无重叠，
  不超过审核保留区。原Z270完整release构建及独立布局检查也通过。
- 构建有39项既有Rust warning，以及主机缺hostname命令的非致命提示；
  不声称零警告构建。

注意：原tools/check_hv_mem_overlap.py不能解析当前模板时会跳过；
本次使用boardgen/elf_check.py及维护目录的verify-layout.py作严格检查。
生成器的严格检查覆盖缺符号、空映射、保留区不足与映射重叠失败路径。

可重复流程：boardgen的integration/verify-build.py，输出目录必须不存在。
本轮证据统一保留于维护目录evidence/baremetal/boardgen-build-strict-20260913。
旧tools/boardgen仍保留作历史副本；后续维护以独立仓库为准。

## 未验收

新主板实机启动、不同CPU/固件的功能适配、跨Zone隔离与设备复位。
真实目标需原生Linux采集、重新审核UUID/内核/initrd/ACPI及保留区，
不能将合成快照或全零UUID平台直接安装到实机。
