# Z270 实机短测与待办（2026-09-13）

## 本轮范围

仅整理文档、执行有界只读/CPU内存检查，未更换hvisor、内核、驱动，
未重启、未写裸盘、未改变设备分配。测试运行于真实Z270的hvisor Zone0。
boot_id：`b039413c-fb08-46eb-8671-f8c29d75512b`。
已部署ELF SHA256：`e5ecf63baf43bc564a25e3f2f45a0145934b983347a88e02804544dfab261eaa`。

## 结果

- 8线程在线、MemTotal32714072kB、19项PCI可见。
- 8进程分别反复SHA256校验128MiB固定数据，运行60秒，全部通过；
  每进程95–102次校验，CPU最高78°C，80°C中止阈值未触发。
  这是约1GiB数据的短测，不是全内存测试，也不是完整内存模式测试。
- /dev/sda与/dev/sdb各从盘首直接只读256MiB到/dev/null，成功完成，
  测得约110/527MB/s；没有读取全盘或验证文件内容。
- 两盘SMART总体PASSED，重分配计数均0；sda待定/不可校正扇区均0。
  sdb历史CRC计数123，本轮前后未增加；sda通电26574小时、
  Load_Cycle_Count556291。保留这些基线用于后续增量观察。
- PPP与Wi-Fi管理IPv6均可SSH连接；PPP SSH完成32MiB零数据传输。
  未做吞吐饱和、断线恢复或长期丢包测试。
- 测试后失败systemd单元0，测试时段内核warning及以上日志无新增。
  NVIDIA580.178.04仍正常，GPU约50°C；未进行GPU压力测试。
- Steam客户端更新完成，steamwebhelper运行；游戏、Proton、登录未验收。
- 本地设备合同检查与两个最高子总线退出回归测试通过。
- GRUB_DEFAULT=0保持原生Arch回退，实验启动方式不变。

测试脚本统一存放于维护目录`hvisor/baremetal/stability-smoke.py`；
远端原始日志为`/root/arch-z270-maintenance/hvisor/evidence/stability-smoke-20260913.log`。

## 待完善（不可视作本次已完成）

1. 有散热监控的长时CPU/RAM、GPU/Vulkan/实际游戏及组合负载测试。
   本次CPU已到78°C，接近保守停止阈值，扩大负载前先确认散热余量。
2. 多次冷启动、热重启及管理网络恢复；本轮没有重启。
3. 声卡实际播放/录音、蓝牙配对、ASMedia USB外设传输和复位。
4. INTx回退与原生ACPI路由差异；SMBus当前仍使用轮询。
5. LPC/PMC电源管理、EFI runtime、休眠/唤醒，当前不作支持承诺。
6. boardgen模板同步、构建入口集成与新名称生成构建已完成，见
   BOARDGEN_INTEGRATION.md；新主板实机验收仍待做。
7. 跨Zone设备复位/交接，以及PCI/EPT/VT-d/IRQ/ACPI一致分配与隔离。
8. Steam代理目前为临时SSH隧道，长期方案尚未配置。

历史文档以NATIVE_TOPOLOGY.md及本记录为准。QEMU板级已有用户改动、
build.rs的boardgen集成和旧tools/boardgen目录未纳入本轮文档提交。
