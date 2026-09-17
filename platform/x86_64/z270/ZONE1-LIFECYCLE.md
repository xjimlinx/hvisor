# Z270 Zone1 在线重启：开发检查点

分支 `feat/z270-zone1-lifecycle`，从 Windows 实验分支 b6d7443 分出。
**在线重启未完成、未实机验证。不能把本分支作为可用 restart 功能。**
2026-09-17 已按用户要求安装安全修复供下次启动使用，未重启整机或后端。
当前运行实例及 sudo 权限未改变。

部署源：hvisor 273c777，hvisor-tool 8a5dbc0；Z270 INFO，关闭固件探针模式。
磁盘上的 SHA256：

- `/boot/hvisor/z270/hvisor`：`6d9bb5cc1cd3c82b825a70fc30c5da2f6d37bd9333a9ec03eb9e92a74c04dda5`
- `/usr/local/lib/hvisor-z270/zone1-virtio-test/hvisor-tool-net`：`0110dbf6628f0fa25546ef2a68b2cbfc697ce8080cdccf3d5966443ac5b63c61`

远端备份和安装脚本集中于
`/root/arch-z270-maintenance/hvisor/candidates/lifecycle-20260917/`，旧二进制
在 `backup/hvisor`、`backup/backend`。GRUB 配置逐字比较未变。
安装使用新文件 rename；后端 PID 506 未变，仍运行旧 inode。
**仅文件部署完成，下一次启动效果未验证。禁止直接 restart 磁盘后端。**

## 已修复的停机前置问题

1. 原 shutdown 等待循环超过 MAX_WAIT_TIMES 后对未停止 CPU 返回 false，
   后续仍可能释放 Zone。改为 x86 用 HPET 五秒期限，超时返回 EBUSY，保留
   Zone/EPT/设备归属；部分停止不是成功，不能自动重新开放镜像加载。
2. x86 idle 先发布 Stopped、再切 VMCS/EPT，存在过早回收窗口；改为切到
   parking EPT 后 Release 发布 Stopped。其他架构停泊时序尚未审查。
3. setup_vmcs 替换旧 VMCS 页之前先 VMCLEAR，避免活动页被回收复用。
4. 启停 x86 lifecycle 使用 try_lock，冲突返回 EBUSY；不持 Zone 写锁和
   virtio IRQ 表锁等待远端 CPU，防止远端 VM-exit 路径锁依赖。
5. 重新配置 CPU 时显式设置 boot_cpu 的 true/false，避免残留 BSP 角色。

测试：32 线程 Z270 INFO 构建、test-device-contract.py、
test-lifecycle-contract.py；最后一个只是源码时序回归检查，不证明实机正确。

## 尚需实现，禁止跳过

### 后端清理进展（2026-09-17）

配套 hvisor-tool 的 `feat/z270-zone1-lifecycle` 分支已提交：

- e024cdb：eventfd 唤醒并 join 事件线程，再释放设备和共享映射；等待
  已进入的回调结束，避免 epoll 关闭后线程继续访问已释放设备。
- 磁盘 worker 退出后执行 fdatasync，EINTR 重试；同步或关闭失败向上传递，
  后端返回失败而不是打印成功。只读设备不执行同步。
- block worker 的 pthread_join 失败时终止后端，禁止继续释放可能被访问
  的资源。这是异常兜底，不是正常停机手段，也不构成写入持久化保证。

`bash tests/test-backend-stop.sh` 覆盖真实事件线程停止、回调等待、反复
初始化/销毁，以及模拟磁盘同步/退出错误；32 线程编译通过。事件线程测试
另通过 ASan/UBSan。这些没有测试真实磁盘或 PCI 复位，**不能据此启用 restart**。
尚缺客体停止提交请求的握手、在途 I/O 排空期限和跨进程代次确认；不能在
客体仍使用根盘时直接停止后端。代码已安装，尚未在运行实例中生效。

本次只读检查：Zone0/Zone1 均列为 running，后端服务 active；但 Zone1
10.77.0.2 邻居状态 FAILED、SSH 超时。不能将该状态当作客体已安全关机。

### 整体协议

重启顺序必须是：

```
能力/配置检查 → 客体停止应用并卸载根盘 → virtio 排空并 fsync
→ 设备停止 DMA/中断 → 全部 vCPU 停泊确认
→ VT-d context/IOTLB/中断路由清理确认 → 设备安全复位
→ 后端退出并释放共享队列/映射 → 授权新一代 staging
→ 重建 EPT/虚拟 LAPIC/IOAPIC/SIPI/virtqueue → 加载镜像 → 启动 → SSH/磁盘验证
```

- 实机 Zone1 拥有 IGD 00:02.0 和 ASMedia 04:00.0（客体 00:1c.0）。
  IGD 不得通过全局 PCH reset 重置；ASMedia 上游 bridge reset 必须确认
  下游全部属于 Zone1。禁止为了恢复 USB/显示而重置 Zone0 共享资源。
- 当前 intel_vtd 的板级 clear_devices 会清 context 和 IOTLB，但没有
  完整设备 quiesce/复位完成证明；通用 remove_device/viommu_remove 还有 todo。
  不能把清 DMA 映射当作硬件已经停止 DMA。
- virtio 后端使用真实分区和常驻共享映射。必须显式停止接收新请求、等待
  在途 I/O、持久化写入、清 IRQ/队列，不能用 kill -9 作为正常重启流程。
- 当前 SEALED 保持 one-shot，故意未解除；只有上述清理全部确认后，
  才能引入代次 token 和重新加载授权。旧驱动/旧工具必须明确拒绝 restart。
- CPU 还需清虚拟 APIC/timer/IRR/ISR、SIPI 地址、事件队列；禁止影响 Zone0。
- 失败保留诊断和资源，不自动重试、不断电、不回退成整机重启。
- 最后再部署 systemd 控制脚本；现有 oneshot 的 restart 不等价于客体重启。

## 验证顺序

先无 PCI/无磁盘 RAM 探针重复启动停止；再最小 Linux；再 virtio 测试镜像；
最后 ASMedia 和 IGD 逐项加入。每步验证 Zone0 网络、图形、USB 不受影响。
真实 Zone1 根盘的在线重启必须最后验证，不能拿用户正在使用的文件系统
作为第一轮停止/复位实验对象。
