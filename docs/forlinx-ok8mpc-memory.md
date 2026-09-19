# Zone0 2.5 GiB memory layout

2026-09-19 实测：当前配置可以启动并登录；Linux MemTotal 约 2.3 GiB，
GPU renderD128 存在。CPU0–1，DDR 地址边界按已观测的 0x140000000 处理。

| Bank（右边界不包含） | 容量 |
|---|---:|
| 0x80000000–0x92400000 | 292 MiB |
| 0x94400000–0xec000000 | 1404 MiB |
| 0x100000000–0x136000000 | 864 MiB |
| 合计 | 2560 MiB = 2.5 GiB |

EL2 自身映射、Zone0 stage-2 和 Linux memory/reg 均覆盖以上三段。
BOARD_PHYSMEM_LIST 元组第二项是结束地址，不是长度；必须覆盖高于
4 GiB 的第三段，否则 dc ivac 会触发 EL2 translation fault。

排除 DSP 0x92400000–0x94400000；保守不使用裸机曾标记 reserved 的
0xec000000–4 GiB 和 0x136000000 以上。reserved 不等同于物理不存在。
0x50000000–0x80000000 的非 Root 预留维持不变；未启动或验证 Zone1。

Hvisor 位于 0x40400000；guest DTB 0xa0000000、内核 0xa0400000；
CMA 256 MiB 的可分配区为 0xb0000000–0xd0000000，低于 4 GiB。
高地址 RAM 可能增加 32-bit DMA 设备的 bounce-buffer 压力，需继续验证
SD/USB/网络负载和 SWIOTLB；启动成功并不代表所有负载已验证。

## 当前板上文件校验

- hvisor.bin: `48609d596bae41dea3a959a27a4dd5c2aeff7b10994d1b203a3ddb85b580b232`
- zone0.dtb: `d6a1241c5c70d784d39a8855029cbaca608c47d034f18d69c0f5c974f64ff5e1`
- hvisor-loader.efi: `5e33a4ead05ca977065856fe17b921b3433ffe40563a8da3b1a36413c42ea5c4`

本次归并没有替换这些已实测文件；旧失败版本的原因和文件历史保存在 Git。
构建和部署入口见 [板级指南](forlinx-ok8mpc.md)。
