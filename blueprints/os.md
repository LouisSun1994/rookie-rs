# OS 藍圖：類 Unix 的家譜、kernel 的六個子系統、七個貫穿模式

> 這是「理解整個 OS」這個方向的地圖，不是日誌。家譜回答 macOS / Linux 各自從哪來、共用什麼；六個子系統是 kernel 的骨架；七個模式是貫穿所有子系統的骨幹——每條新挖的線都應該能掛回其中一個子系統、一個模式。
> 學習的點滴仍記在 `notes/`；每條線挖到哪裡、下一條挖什麼、還有哪些題沒答，記在 [`os-checklist.md`](os-checklist.md)。
>
> 建立日期：2026-11-05｜素材：note 05 產出後的追問（2026-11-04 ～ 11-05），從「Windows 上怎麼做」一路挖到 Mach 與類 Unix 家譜；概念本身記在 note 05 §9–§13。
> 維護原則：模式表的「已在哪裡撞到」欄每挖完一條線補 note / 經驗編號；家譜與子系統表只在模型被校正時改。標（入庫時補）的是整理時加的、未經導師確認。

---

## 1. 家譜：類 Unix 是共用介面，不是共用程式碼

```
1969  Unix（Bell Labs，Thompson & Ritchie）         fork / exec / fd / pipe 在這裡定型
  │
  ├─ 1977– BSD（Berkeley）                            加 socket、虛擬記憶體、vnode、signal 重做
  │     ├─ 1993 FreeBSD / NetBSD / OpenBSD（BSD 授權）
  │     └─ 1985 Mach（CMU）：BSD 當殼，底下換 microkernel（task / thread / port）
  │            └─ 1989 NeXTSTEP = Mach + BSD + Objective-C 框架
  │                  └─ 1997 Apple 買 NeXT → 2000 Darwin 開源（XNU = Mach + BSD + IOKit）→ 2001 Mac OS X
  │
  ├─ 1983– System V（AT&T 商業線）→ Solaris、AIX、HP-UX
  │
  ├─ 1988 POSIX（IEEE）：只規定介面（函數簽名與語意），不規定實作
  │
  └─ 1987 Minix（教學用）
        └─ 1991 Linux：從零寫，只參考介面 → 1996 發明 clone → 2.6 起 NPTL / epoll / cgroups / namespaces
```

- macOS = 真的 BSD 程式碼 + 真的 Mach 程式碼；Linux = 從零重寫、長得像。兩者沒有共用任何一行。Linux 比 macOS 早十年，**不是**根據 XNU 調出來的。
- 授權決定了誰能拿誰的碼：BSD 授權允許 Apple 拿去閉源改（macOS userland 多來自 FreeBSD）；GPL 不允許。macOS 2019 年預設 shell 從 bash 換 zsh，是因為 bash 4+ 改 GPLv3——Claude Code 的 Bash 工具跑 zsh，根源是授權。
- Apple 開源的是 Darwin：XNU kernel（`github.com/apple-oss-distributions/xnu`）、libc、launchd、部分 userland。閉源的是 framework、大多數 kext（Sandbox.kext 在內）、GUI。所以「kernel 開源、policy 閉源」。
- Windows NT 不在這棵樹上：另一個血統（VMS 系），安全模型是 identity-based 不是 policy-based；與 Seatbelt 的逐項對照、WSL2 怎麼把一顆真 Linux kernel 塞進來，見 note 05 §10（入庫時補）；WSL2 那台 VM 本身——utility VM、hypervisor 與 VT-x、VM / container / sandbox 三層——見 note 06。

## 2. 一個類 Unix kernel 的六個子系統

```
                         user space
      libc / libpthread / libxpc / dyld(ld.so) / shell / launchd(systemd)
═════════════════════════ 一扇門：trap ═════════════════════════════════
 ①process 管理    ②虛擬記憶體     ③檔案系統      ④IPC 與網路      ⑤安全        ⑥裝置
 proc/task/thread  vm_map/頁表     VFS/vnode      pipe/socket      ucred/DAC    driver
 fork/exec/spawn   page fault      inode/dentry   signal/Mach port  MAC/label   ioctl/DMA
 scheduler/wait    COW/mmap/brk    page cache     epoll/kqueue     capability  中斷
 session/tty/pgrp  swap/TLB        mount/namespace futex            namespace/cgroup
═══════════════════════════════════════════════════════════════════════
                硬體：特權模式、MMU、中斷控制器、timer、PCIe
```

## 3. 七個貫穿所有子系統的模式

| # | 模式 | 一句話 | 已在哪裡撞到 |
|---|---|---|---|
| 1 | 物件 + 引用 = capability | fd→fileglob、port name→port right、HANDLE→object；引用 per-process、物件全域有 refcount；檢查在「拿」不在「用」 | note 05 §6 fd / SCM_RIGHTS、§13 Mach port |
| 2 | 一扇門 | user / kernel 只有 trap 一個入口（syscall、中斷、page fault）；「誰讓它動」追到底是這三種之一 | note 02 中斷；note 05 §12 trap；經驗 01 §4 門的長相由 ISA 定；note 06 §6 guest kernel → hypervisor 的 VM exit 是同一扇門再往下一層 |
| 3 | 樹狀繼承 vs 橫向傳遞 | ucred、fd table、cwd、env 沿 process tree 往下複製；橫向只能走 IPC 且 kernel 蓋章 | note 05 §5 label 血緣、§7 audit token、§11 繼承表 |
| 4 | 介面 vs 實作 | POSIX 規定 `fork()` 長什麼樣，不規定是 syscall 還是 clone 的包裝 | note 05 §11 clone vs fork / spawn；經驗 01 §3 API vs ABI、§9 同一個 POSIX API 兩家 libc |
| 5 | 機制 vs 政策 | kernel 提供機制（MAC hook、context switch、page 替換），政策另外餵（SBPL、nice、LRU） | note 05 §2–§3 Seatbelt |
| 6 | 名字空間 | 路徑、pid、port name、mount 都是「名字→物件」的表；Linux namespace = 每 process 一份表 → 容器 | note 05 §10 bubblewrap ENOENT vs Seatbelt EPERM；經驗 01 §7 PT_INTERP 路徑、符號名 → `.so` |
| 7 | 快取與一致性 | page cache、TLB、dentry cache；「寫入了 / 落盤了」「對映了 / 在 RAM」是不同狀態 | note 02 寫檔 |

反覆卡住的三個點對應：「幻想中央管理者」→ 模式 2；「user/kernel 層混放」→ 模式 4；「等待 vs 在佇列」→ 模式 2。

## 4. 資源（官方 / 第一手優先）

- **OSTEP**（*Operating Systems: Three Easy Pieces*，免費）：概念優先、每章一個機制、附可跑小程式；先讀 Virtualization（CPU 與 memory）。
- **Ritchie & Thompson 1974〈The UNIX Time-Sharing System〉**：15 頁，作者解釋 fd / fork / exec / pipe 為何長這樣。接 Ritchie 1979〈The Evolution of the Unix Time-sharing System〉。
- **xnu 原始碼**：`bsd/kern/kern_fork.c`、`kern_exec.c`、`bsd/sys/proc.h`、`ucred.h`。
- **Linux 原始碼**（elixir.bootlin.com）：`kernel/fork.c` 的 `copy_process()` 就是「複製 / 共用 / 重設」那張表。
- Kerrisk《The Linux Programming Interface》（查用）；Levin《*OS Internals》Vol. 1（macOS / Mach 對應物）。
- 1986 Mach 論文〈Mach: A New Kernel Foundation for UNIX Development〉：port / task / thread 的原始設計理由。
