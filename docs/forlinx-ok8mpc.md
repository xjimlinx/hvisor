# Forlinx OK8MP-C：已验证配置入口

适用开发板：飞凌 OK8MPlus-C / OK8MP-C（i.MX8MP），不是 Orange Pi。
本分支保留 upstream 多平台源码，不为“干净”删除无关平台或改写 Git 历史。

## 当前验证边界

- Hvisor Root Linux 使用 CPU0–1；不是四核全直通配置。
- HDMI 出画面、USB Host 枚举 Hub/键鼠、串口登录已实测。
- GRUB → EFI R4 → Hvisor → Linux 已实测；R4-fast 搬运等待明显缩短，
  尚无精确耗时和长期稳定性数据。
- GPU 的 GC7000/GC520 驱动绑定和 renderD128 已确认；桌面渲染性能
  未定量验证。Type-C 双角色、无线 AP 不计入此稳定基线。
- KDE 出现过 OOM：可管理内存约 1.14 GiB、无 swap；尚未解决。
- 当前 DTS 仍含旧 USB 节点，可能出现不影响已验证 Host 枚举的
  deferred-probe 报错；本次目录整理没有偷偷变更设备树。

## 唯一推荐入口

| 内容 | 路径 |
|---|---|
| 板级资源与链接配置 | `platform/aarch64/forlinx-ok8mpc-hdmi/` |
| 可独立编译的完整 Zone0 DTS | `platform/aarch64/forlinx-ok8mpc-hdmi/zone0-hdmi-usb-r1.dts` |
| GRUB EFI 加载器、引导配置与部署说明 | `tools/ok8mp-efi-loader/README.md` |
| HDMI/USB 增量生成工具 | `tools/ok8mp_hdmi_dts.py`、`tools/ok8mp_usb_dts.py` |
| 历史验证记录 | `artifacts/forlinx-ok8mpc-hdmi-wifi/` |
| 无 HDMI 的早期回退配置 | `platform/aarch64/forlinx-ok8mpc-core/` |

命名沿用已部署版本，避免仅为了统一名字而让现有启动命令失效。
生成工具依赖外部原始 DTB；直接复现当前基线用已提交的完整 DTS。

新增桌面扩容候选：`platform/aarch64/forlinx-ok8mpc-desktop/`，保留 GPU，
分三段提供 Zone0 2.5 GiB；[地址规划与验证限制](forlinx-ok8mpc-memory.md)。
候选已部署，扩容启动尚未验证，不替换上表的已验证回退基线。

## Build

在仓库根目录执行（需要项目 Rust 工具链、交叉 binutils、mkimage、dtc）：

```sh
make BID=aarch64/forlinx-ok8mpc-hdmi gen_cargo_config
make BID=aarch64/forlinx-ok8mpc-hdmi target/aarch64-unknown-none/release/hvisor.bin
mkdir -p target/ok8mpc
dtc -I dts -O dtb -o target/ok8mpc/zone0.dtb platform/aarch64/forlinx-ok8mpc-hdmi/zone0-hdmi-usb-r1.dts
make -C tools/ok8mp-efi-loader verify
```

编译成功不等于硬件验证通过。部署须保留裸机默认启动和上一个实测版本，
先上传到独立目录，校验后添加非默认菜单，不覆盖运行中的基线。

## Rootfs 清理原则

现有 Arch/KDE 是开发系统，不等同于可分发镜像。清理前先取得磁盘空间、
启动配置、包列表和文件清单；默认保留用户文件、桌面、网络配置、内核
对应模块/固件、SSH 和两个回退引导路径。

实验镜像和旧 EFI/DTB 只在确认不被 GRUB/U-Boot 引用后归档到主机，
校验归档可读取后再考虑移除板上副本。不要直接清空 /boot、/root、
/home、/usr/lib/modules 或 pacman 包缓存。

若目标是对外发布干净 rootfs，应在独立副本中去除凭据、SSH host keys、
machine-id、网络秘密和个人文件，并重新生成首次启动逻辑；不得在当前
通过 SSH 管理的运行系统上直接执行这些去身份化操作。
