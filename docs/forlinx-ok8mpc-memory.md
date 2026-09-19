# Zone0 2.5 GiB desktop candidate

首次扩容启动在 FAR=0x100000000、ELR=0x40418a70 失败；反汇编为
`dc ivac, x8`。BOARD_PHYSMEM_LIST 第二项是结束地址而非长度，旧 EL2
映射止于 4 GiB。现修正桌面板级 EL2 表，覆盖全部三段 guest RAM，
继续排除 DSP、0xec000000–4 GiB 和顶部区域。DTB/GRUB 不变，需重测。

2026-09-19：当前旧内存配置下 Etnaviv GC7000/GC520 已绑定，出现
`/dev/dri/renderD128`。本候选保持该 GPU/HDMI/USB DTS，仅修改 RAM banks。
两核不变，扩容不等于解决所有 OOM 或证明长期稳定。

| Zone0 bank（右边界不包含） | 容量 |
|---|---:|
| 0x80000000–0x92400000 | 292 MiB |
| 0x94400000–0xec000000 | 1404 MiB |
| 0x100000000–0x136000000 | 864 MiB |
| 合计 | 2560 MiB = 2.5 GiB |

Hvisor stage-2 映射和 Linux memory/reg 使用相同的三段，不跨洞映射。
保持 0x50000000–0x80000000 非 Root 预留；排除 DSP 0x92400000–0x94400000。
保守排除此前裸机 /proc/iomem 显示 reserved 的 0xec000000–0x100000000，
以及 DDR 顶部 0x136000000 以上，避免此前观测到的 0x13b000000 起固件区。
reserved 不一定意味着硬件不存在，此处是保守不使用，而非宣称物理空洞。
不采信旧 mainline DTS 中超出已观测 0x140000000 DDR 边界的声明。

Hvisor 在 0x40400000，guest DTB 在 0xa0000000、内核在 0xa0400000；
CMA 保持 256 MiB，位于低于 4 GiB 的 0xb0000000–0xd0000000 范围。
高于 4 GiB 的 RAM 可能增加 32-bit DMA 设备对 bounce buffers 的需求；
启动后需验证 SD/USB/FEC 和图形负载，无 SWIOTLB exhausted、SError。
`free` 的 MemTotal 会比 2.5 GiB 略少，不保证等于该数字。

构建 BID：`aarch64/forlinx-ok8mpc-desktop`；DTS 在同目录 `zone0.dts`。
EFI 构建：`make -C tools/ok8mp-efi-loader build/hvisor-desktop-2g5.efi`。
部署目录：`/boot/hvisor-profiles/desktop-2g5/`。独立非默认 GRUB 项，
旧内存版及裸机入口保留。部署校验不代表扩容已实测。

首次失败候选的部署 SHA256（Hvisor 需要下面描述的 EL2 修正）：

- hvisor.bin: `195ca0c8bacc83002158f59366dbb6151276363058690f2d85751cf3cb4cb218`
- zone0.dtb: `d6a1241c5c70d784d39a8855029cbaca608c47d034f18d69c0f5c974f64ff5e1`
- hvisor-loader.efi: `5e33a4ead05ca977065856fe17b921b3433ffe40563a8da3b1a36413c42ea5c4`

静态检查：三个 RAM bank 合计 0xa0000000 字节且不与排除区相交；
与 GPU 基线相比 DTS 仅 memory 节点变化。首次启动后检查 `free -h`、
`/proc/iomem`、`lsusb`、`/dev/dri` 及 SSH，再确认 SD/USB 数据读写无错误。
