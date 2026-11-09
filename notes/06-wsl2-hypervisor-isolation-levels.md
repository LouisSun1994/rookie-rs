# Note 6 — WSL2、hypervisor 與隔離三層級：一台 VM、若干容器、三條線

> 這份筆記記錄的是「WSL2 具體是什麼」這條線：從 `wsl -d Ubuntu` 背後是一台 VM 還是一個容器起步，往下挖到 Hyper-V、hypervisor 與 CPU 的第三層特權模式、雲上的 VM 其實是同一種東西、Windows 上四種執行情境各跑在哪顆 kernel、namespace 是按子系統各一張表、以及 VM 什麼時候活什麼時候死。
> 核心收穫是四句話：**WSL2 是一台 utility VM 加若干容器**——所有 distro 共用一顆 kernel，distro 只是 namespace + 自己的 vhdx；**hypervisor 不模擬**，guest 指令原生跑在 CPU 上，它只切硬體、擋特權動作；**隔離三層級是三條畫在不同位置的線**，疊加不是三選一；**VM 活著是結果不是原因**，有 process 才有 distro、有 distro 才有 VM。
>
> 記錄日期：2026-11-09｜當時定位：🥚 小白（仍未動手寫程式），OS / systems 支線。接在 [note 05 §10](05-macos-sandbox-seatbelt-proc-cred.md)（Windows 與 WSL2 對照）之後往下挖；也接 [經驗 01](../experiences/01-libc-abi-alpine.md)「跨 OS 必死，除非有人補一顆 kernel」那句——這份講的就是那顆 kernel 怎麼補進來。藍圖 ⑤ 安全、④ IPC；模式 2（一扇門再往下一層）、模式 6（名字→物件表，按子系統各一張）。
> 時效：WSL 於 2025-05 開源（`microsoft/WSL`）；mirrored 網路 2023 起；Docker Desktop 預設 WSL2 backend；Claude Code 在 native Windows 不開 sandbox、WSL2 走 bubblewrap。皆為 2026-11 時的狀態。
> 標「補強」的 blockquote 是整理進 repo 時加註、尚未經導師確認的延伸；其中標「推論」的連整理者都還沒查證，只是依模型推出來的。

---

## 0. 一句話版本

WSL2 是 Hyper-V 開出來的一台 utility VM，裡面一顆 Microsoft build 的真 Linux kernel，pid 1 是 WSL 自己的 `/init`；每個 distro 只是這台 VM 裡的一組 namespace 加一顆 ext4.vhdx。隔離的線有三條、畫在三個不同位置：hypervisor 在硬體之上、kernel 之下（不共用 kernel）；container 在 kernel 之上、process 之下（共用 kernel）；sandbox-runtime 在同一棵 process tree 內（共用幾乎一切）。三條線疊加不是三選一，WSL2 上跑 Claude Code 三條全在。hypervisor 不模擬 guest：指令原生跑，只是 CPU 多了一層特權模式（VT-x），guest kernel → hypervisor 的 VM exit 跟 user → kernel 的 trap 是同一扇門再往下一層。

---

## 1. WSL2 是什麼：一台 VM + 若干容器

一句話：Hyper-V 開出來的 utility VM（無 UEFI、無模擬裝置、直接把 Microsoft build 的 Linux kernel 載進記憶體開機），pid 1 是 WSL 自己的 `/init`；**所有 distro 共用這一台 VM、這一顆 kernel**，每個 distro 只是 VM 裡的一組 namespace + 自己的 ext4.vhdx 當 rootfs。

```
Windows user      wsl.exe（CLI）             解析參數，跟 wslservice 講話，接 console
                  wslservice.exe             管理者：開 VM、管 distro、跑 9P server、處理 interop
                  wslhost.exe / relay        搬 Linux pty ↔ Windows console 的 bytes
Windows kernel    Hyper-V hypervisor         切 CPU / 記憶體給 VM（Windows 自己是 root partition）
                  HCS（Host Compute Service）建 utility VM 的 API，不經 Hyper-V 管理員
                  VMBus                      host ↔ VM 通道：virtual SCSI（vhdx）、synthetic NIC、hvsocket
═══ VM 邊界 ═══
Linux kernel      linux-msft-wsl（MS fork：dxgkrnl、hvsocket、9P 調校）
                  pid 1 = /init（WSL 自寫，不是 systemd）
                  ├─ system distro（CBL-Mariner 基底，跑 WSLg 等，使用者看不到）
                  ├─ distro Ubuntu      自己的 mount / pid namespace、自己的 vhdx 當 /
                  ├─ distro Debian      同上；同一顆 kernel、同一份記憶體池、**同一個 network namespace**
                  └─ distro docker-desktop（Docker Desktop 裝的，裡面跑 dockerd）
Linux user        bash、git、Claude Code、bubblewrap…
```

- `wsl -d Ubuntu` 不是開一台 VM，是請 `/init` 在已經在跑的 VM 裡建一組 namespace、掛上 Ubuntu 的 vhdx、起一個 shell。
- distro 之間的隔離**刻意很弱**（同一使用者、共用 netns、共用 /mnt/c）：Microsoft 要的是方便，安全邊界是 VM 那條線。
- WSL1 是另一種東西：NT kernel 裡的 `lxcore.sys` 翻譯 Linux syscall（pico process），跟 Seatbelt 一樣住在 syscall 邊界；翻譯層補不完（namespace、seccomp、io_uring、Docker 全卡）才改成 WSL2 真 kernel。

> **補強**（開源的範圍）：2025-05 開源的是 user space 那堆——`wsl.exe`、`wslservice`、`/init`、Plan9 server、各種 relay——Linux kernel 的 fork（`WSL2-Linux-Kernel`）則 2019 年起就一直是開的。仍留在 Windows 裡閉源的兩個：`lxcore.sys`（WSL1 的 syscall 翻譯層）與 `p9rdr.sys`（Windows 側讀 `\\wsl$` 用的 9P redirector 檔案系統 driver）。所以上面這張圖裡，Windows kernel 那一列與 Linux kernel 那一列現在都讀得到 source，差在 kernel driver 那兩支。

## 2. 時間軸：`wsl.exe` 到 prompt

```
t0  Windows user   wsl.exe                 解析 -d Ubuntu；經 RPC 找 wslservice
t1  Windows user   wslservice              VM 沒起 → 讀 .wslconfig（CPU 數、記憶體上限）
t2  Windows kernel HCS + Hyper-V           建 utility VM：vCPU、記憶體、VMBus 通道；無 UEFI、無模擬裝置
t3  Windows kernel HCS                     把 C:\Windows\System32\lxss\tools\kernel 直接載進 VM 記憶體，附 cmdline，跳進去
═══ VM 邊界 ═══
t4  Linux kernel   start_kernel            探到 VMBus 上的 SCSI、NIC、hvsocket
t5  Linux kernel   mount system distro      掛 system vhdx 當根；exec /init 成 pid 1
t6  Linux user     /init                   hvsocket 連回 wslservice；掛 /mnt/c（9P）、設 vEthernet、產 /etc/resolv.conf
t7  Linux user     /init                   為 Ubuntu 建 namespace（mount、pid…）、掛它的 vhdx 當 /、起 distro init（或 systemd）
t8  Linux user     distro init             開 pty，fork+exec bash -l
═══ VM 邊界 ═══
t9  Windows user   wslhost.exe             經 hvsocket 拿 pty 另一端，跟 console 搬 bytes → 看到 $
```

第二個 distro 只重跑 t7–t9。`wsl --shutdown` 關整台 VM，所有 distro 一起死。

> **補強**（t3 的 kernel 路徑）：`System32\lxss\tools\kernel` 是 Windows 內建那版 WSL 的位置；Store / MSIX 版（WSL 1.0 之後的主線）把整包裝在 `%ProgramFiles%\WSL\`，kernel 也跟著搬（推論：`%ProgramFiles%\WSL\tools\kernel`，未在機器上確認）。不管在哪，`.wslconfig` 的 `[wsl2] kernel=` 可以指到自己 build 的 kernel 檔——這正是「kernel 是一個被 HCS 直接載進記憶體的檔案」最直接的證據：沒有 bootloader 去找它，是 host 把它放好再跳進去。

## 3. 時間軸：bash 裡跑 `notepad.exe`（interop = 逃逸點）

```
t0  bash (Linux)       fork + execve("notepad.exe")
t1  Linux kernel       binfmt_misc：檔頭 "MZ"（PE）→ 登記表說交給 /init      ← 經驗 01 的②格式層被「代打」
t2  /init (Linux)      打包「程式、參數、cwd、env」經 hvsocket 送 wslservice
═══ VM 邊界 ═══
t3  wslservice (Win)   翻譯 cwd（/mnt/c/… → C:\…）、CreateProcess；stdio 接 pipe
t4  notepad (Win)      Windows process、Windows token；不在任何 Linux namespace / seccomp 裡
t5  wslservice ↔ /init  stdio 經 hvsocket 來回；退出 → exit code 回 bash
```

與 launchctl 同形（note 05 §7）：寄信請外面的人開程式。bubblewrap 的解法同形：sandbox 內看不到 `/init` 的 socket，t2 寄不出去。

> **補強**（t2 那封信寄到哪、怎麼被擋）：每個 distro session 有一個 unix socket `/run/WSL/<n>_interop`，路徑放在環境變數 `WSL_INTEROP`；t1 binfmt_misc 把 `.exe` 交給的 interpreter 是 `/init`，`/init` 讀 `WSL_INTEROP` 連那個 socket 把請求送出去。所以 bubblewrap 要擋 interop 有兩個互相獨立的手：mount namespace 裡不 bind `/run/WSL`（socket 路徑在 namei 就 ENOENT——跟 note 05 §10「用 mount ns 讓路徑不存在」同一招）、以及清掉 `WSL_INTEROP`。兩個都不做，`.exe` 照跑且不在 sandbox 內。`/etc/wsl.conf` 的 `[interop] enabled=false` 是 distro 層的總開關，關的是 binfmt_misc 登記本身。

## 4. 三條跨 VM 管線、網路、GPU

| 管線 | 協定 | 誰當 server | 細節 |
|---|---|---|---|
| Linux 讀 `/mnt/c` | 9P over hvsocket，mount type `drvfs` | **Windows 側** wslservice 的 Plan9 server，用你的 **Windows token** 開檔 | 每個 open / stat 一次跨 VM RPC → `git status` 慢一個數量級；Linux mode bits 靠 metadata 選項模擬 |
| Windows 讀 `\\wsl$\Ubuntu\` | 9P 反向 | **Linux 側** /init 的 9P server | Explorer / VS Code 走這條 |
| 控制通道 | AF_VSOCK（Linux）↔ AF_HYPERV（Windows） | 雙向 | interop、9P 承載、pty、WSLg 的 RDP 流 |

教訓：檔案放哪邊就在哪邊處理；跨邊界每次 I/O 都是 RPC。

```
網路 NAT 模式（預設）   vEthernet (WSL) 虛擬交換器，VM 拿 172.x 私有 IP；localhost forwarding 補丁把 VM 內 bind 的 port 轉到 Windows
     mirrored 模式（2023+）VM 鏡像 Windows 的介面與 IP；localhost 兩邊相同；VPN / 防火牆行為一致
GPU  /dev/dxg + dxgkrnl（paravirtual）：Linux CUDA → /dev/dxg → VMBus → Windows GPU driver → GPU（note 03 的 GPU 路多一層 VM 邊界）
GUI  WSLg：system distro 跑 Weston + RDP server，Windows 用 mstsc 畫每個視窗
```

> **補強**：`\\wsl$\` 的新名字是 `\\wsl.localhost\`，兩個都能用；Windows 側替這條路做檔案系統的就是 §1 補強提到的 `p9rdr.sys`。把這張表跟 note 05 §10 的三行版對照：那邊只寫了代價，這邊補了「誰當 server、用誰的身分」——`/mnt/c` 那列的「Windows token」是 §12 Q2 與 Q12 的全部答案。

## 5. 隔離三層級：線畫在不同位置

```
                    hypervisor（VM）              container（namespace + cgroup）     sandbox-runtime（bwrap + seccomp）
─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
誰畫的線             CPU 硬體 + hypervisor          Linux kernel                        Linux kernel
線畫在哪             硬體之上、kernel 之下           kernel 之上、process 之下            同一個 process tree 內
隔離單位             一整個 OS（自己的 kernel）       一棵 process tree                   一次 Bash 呼叫的 process tree
guest 看到什麼       「我有一台電腦」                「我有一個系統」但 kernel 是別人的     「我有一個縮水的系統」：白名單路徑、沒網卡、某些 syscall 拒絕
共用什麼             只共用實體硬體                  kernel、page cache                   kernel、使用者、大部分檔案系統
隔離強度             最強（kernel 漏洞不跨 VM）       中（kernel 漏洞 = 全部淪陷）           弱（只擋「不該做」，不擋「看得到」）
成本                 記憶體獨佔、秒級                 毫秒、幾 MB                          毫秒、幾乎零
被擋時的樣子         根本沒有那個裝置                 ENOENT、看不到那個 pid               ENOENT（mount ns）或 EPERM（seccomp）
```

三者疊起來，不是三選一。WSL2 上跑 Claude Code 三層全在：

```
Hyper-V                                 ← 線 1：Windows 世界 | Linux 世界
  └ WSL2 VM, Linux kernel
      └ distro Ubuntu                   ← 線 2：這個 distro | 其他 distro / docker-desktop
          └ Claude Code (Node, user space)
              └ sandbox-runtime：bwrap 建 mount/net/pid ns + seccomp    ← 線 3：這次指令 | 其他
                  └ bash -c '...'
```

- sandbox-runtime 是 Anthropic 的 npm 套件，跑在 user space，跟 Seatbelt 線同構：harness 出題、kernel 機制（namespace、seccomp）執行（note 05 §3、§9）。
- Docker 在這裡不是替代品：Windows 上 Docker Desktop 的 Linux 容器自己住在 WSL2 VM 的 `docker-desktop` distro 裡；「把 agent 放進容器」是可疊加的第二層，跟第三層不互斥。

比喻（政府大樓）：hypervisor = 另蓋一棟樓、自己的政府；container = 同一棟樓給你一層、自己的門牌系統（pid ns）與水電配額（cgroup）；sandbox-runtime = 同一間辦公室，給這一個員工一張「只能開這幾扇門、不能打這幾支電話」的清單。

## 6. hypervisor 與 CPU 模式：不是模擬，是原生執行

hypervisor = 「kernel 的 kernel」：把一台實體機切成多台看起來完整的機器，對 guest kernel 做 kernel 對 process 做的事。

| 資源 | 怎麼分配過去 | 真的硬體？ |
|---|---|---|
| CPU | hypervisor 排程 vCPU 到實體 core，guest 指令**原生執行** | 真的，時間片被排 |
| 記憶體 | 劃一塊實體 RAM，用 CPU 第二層頁表（Intel EPT / AMD NPT）讓 guest 以為「這是我的 0～N GB」 | 真的，位址被重新對映 |
| 裝置 | 多半**假的**：paravirtual 裝置（VMBus / virtio），guest driver 把請求送 host 代做 | 除非 PCI passthrough |

```
特權等級          誰住          往下一層的門
──────────────────────────────────────────────────────────
user mode         你的程式       syscall / trap → kernel
kernel mode       guest kernel   VM exit → hypervisor          ← 2006 起 CPU 多的一層：Intel VT-x、AMD-V、ARM EL2
hypervisor mode   hypervisor     —
```

```
t0  guest Linux (VMX non-root)   跑一般指令、存取記憶體       CPU 原生執行；EPT 硬體翻位址；hypervisor 不在場
t1  guest Linux                  driver 寫「網卡」暫存器       paravirtual → 寫 VMBus ring buffer + hypercall
t2  CPU                          VM exit                     存 guest 狀態，切 VMX root，跳進 hypervisor
t3  Hyper-V                      看是哪種 exit → 通知 root partition 的 vmwp
t4  Windows root (kernel/user)   vmwp 讀 ring buffer，交給實體網卡 driver
t5  Hyper-V                      VM entry：恢復 guest，回 t1 下一條
t6  guest Linux                  繼續，以為剛寫了一張真網卡
```

- 「模擬」（emulation：qemu-user、Rosetta）是逐條翻譯指令，慢 5–20 倍，只在 ISA 不同時需要；「虛擬化」（virtualization）是把硬體切一塊給你，用 CPU 硬體擋住不該碰的。WSL2 的 x86-64 kernel 跑在 x86-64 上，零翻譯。
- 一扇門模型再往下一層：user → kernel 靠 trap；guest kernel → hypervisor 靠 VM exit（藍圖模式 2）。
- 為什麼需要這層：kernel 對 process 的隔離前提是 kernel 可信；要同時跑多個**不互信的 kernel**（雲上不同客戶、Windows 旁的 Linux、受保護的憑證區）就需要比 kernel 更低、更小的仲裁者。hypervisor 本體幾 MB，比 kernel 小一到兩個數量級——「kernel 漏洞不跨 VM」的物理基礎。

> **補強**（EPT 為什麼是分水嶺）：VT-x / AMD-V（2005–2006）先只解決「特權指令要 trap 到 hypervisor」；記憶體那層一開始沒有硬體幫忙，hypervisor 得自己維護一份 shadow page table，guest kernel 每改一次頁表就一次 VM exit，早期 VM 慢主要慢在這。第二層頁表（AMD NPT 2007、Intel EPT 2008）讓 CPU 的 MMU 直接做「guest 實體位址 → 真實體位址」的翻譯，guest 改頁表不再 exit。上表「記憶體：真的，位址被重新對映」靠的就是它；沒有 EPT 的話 t0 那行「hypervisor 不在場」不成立。

## 7. Hyper-V：Microsoft 的 type-1 hypervisor

```
開 Hyper-V 之前                      開 Hyper-V 之後（WSL2 / Docker Desktop / Credential Guard 任一開啟）
──────────────────────────────────────────────────────────────────────────
硬體                                  硬體
  └ Windows kernel（最底）              └ Hyper-V hypervisor（最底，bootloader 先載它）
      └ 你的程式                            ├ root partition：Windows 本身 ← 變成一台特權 VM
                                            ├ child partition：WSL2 utility VM
                                            ├ child partition：Windows Sandbox
                                            └ child partition：Hyper-V 管理員建的 VM
```

- 開了 WSL2 之後你的 Windows 自己就在 VM 裡跑（root partition 有特權：直接控制實體裝置、管其他 partition）。
- root / child 不是「之後才有的階段」，是 type-1 設計：hypervisor 太小不含 driver，指定一個特權 partition 替它管實體裝置。Xen 叫 dom0 / domU；KVM 讓 host Linux kernel 兼任 hypervisor 和 root。
- 沒開 Hyper-V 之前沒有 VM 這回事，kernel 分配的是 process 的資源。

| 零件 | 做什麼 | WSL2 用在哪 |
|---|---|---|
| hypervisor（`hvix64.exe` / `hvax64.exe`） | 排 vCPU、管 EPT、處理 VM exit | 底座 |
| root 裡的 VMM 服務（`vmms`、`vmwp`） | 每台 VM 一個 worker process，裝置中繼 | utility VM 的 worker |
| VMBus | root ↔ child 的 paravirtual 通道（共享記憶體 ring + 中斷） | SCSI、NIC、hvsocket 架在上面 |
| HCS | 程式化建輕量 VM / 容器 | `wslservice` 開 utility VM |
| Integration services（Linux `hv_*` driver） | guest 裡的 VMBus driver | MS kernel fork 內建 |

VBS（Virtualization-Based Security）：Windows 把 LSASS 憑證放進另一個 partition，kernel 被攻下也拿不到——「kernel 漏洞不跨 VM」的官方用法。

> **補強**（工作管理員裡那個 `vmmem`）：Windows 上 VM 的記憶體與 CPU 用量顯示在一個叫 `vmmem`（WSL 的叫 `vmmemWSL`）的「process」上。它不是真的 process，沒有映像、殺不掉，是 NT kernel 替 child partition 記帳用的佔位物件——hypervisor 分出去的記憶體總得掛在 root partition 的某個名字下讓工具看見。這是「VM = host 上的一個 process」（§8）在 Windows 的長相；KVM 那邊則是一個真的 qemu / VMM process。ARM64 Windows 的 hypervisor 檔名是 `hvaa64.exe`。

| hypervisor | 誰做 | 類型 | 你會在哪碰到 |
|---|---|---|---|
| Hyper-V | Microsoft | type-1 | WSL2、Docker Desktop (Win)、Windows Sandbox、Azure |
| KVM | Linux 社群 | type-1（kernel module，Linux kernel 自己變 hypervisor；每台 VM 是一個 process） | GCP、大多數雲、`qemu -enable-kvm` |
| Xen | 社群 / Citrix | type-1，Linux 當 dom0 | 早期 AWS |
| Firecracker | AWS | KVM 基底 microVM，無裝置秒開（同 WSL2 utility VM 思路） | Lambda、Fargate |
| ESXi | VMware | type-1 | 企業機房 |
| Hypervisor.framework / Virtualization.framework | Apple | type-1（XNU 內） | Docker Desktop (Mac)、UTM |
| VirtualBox、VMware Workstation | — | type-2（host OS 上的程式） | 桌面 |
| QEMU（不帶 KVM） | 社群 | **模擬器，不是 hypervisor** | 跨 ISA |

type-1 / type-2 只差 hypervisor 在 host OS 底下還是上面；現代都用同一套 VT-x，效能差距小。

## 8. 雲上的 VM 是同一件事

```
                GCP                              AWS（Nitro）                         Azure
──────────────────────────────────────────────────────────────────────────────────────────────
hypervisor       KVM（Linux kernel 內）             KVM 衍生，裝置虛擬化卸載到 Nitro 硬體卡  Hyper-V
host / root      Linux + Google 自寫 VMM（取代 qemu） 極小 Linux + Nitro 韌體                Windows root partition
你的 VM          一個 child                        同                                   同
```

- 一台實體機**一個** hypervisor，切成**多台 VM** 給客戶；客戶拿到 VM，不是 hypervisor。
- GCP 上的 n2-standard-4 = Google 某台 Linux 主機上的一個 process；4 vCPU = 那個 process 的 4 條 thread 被 KVM 排到實體 core；RAM = EPT 對映的一塊。跟 WSL2 utility VM 同一種東西，只是你是客戶、Microsoft 是房東。
- 在 GCP VM 裡再開 WSL2 / Docker Desktop 要開 nested virtualization：hypervisor 上再跑 hypervisor，VMX 要被虛擬化一次。

## 9. Windows 上四種執行情境

| 情境 | 要 VM？ | 跑在哪顆 kernel | 隔離線畫在哪 | VM 什麼時候起 |
|---|---|---|---|---|
| Claude Code native Windows | 否 | NT | 沒有（只有 harness permission prompt） | — |
| Claude Code sandbox | 是 | WSL2 VM 的 Linux | VM 裡某個 distro 裡，每次 Bash 再用 bwrap + seccomp 畫一條 | 第一次 `wsl` 時起，閒置自動關 |
| Docker **Linux** 容器 | 是 | 同一台 WSL2 VM | `docker-desktop` distro 裡，每容器一組 namespace | Docker Desktop 啟動時起，它讓 VM 一直活 |
| Docker **Windows** 容器 | 可選 | process isolation：host 的 NT；Hyper-V isolation：每容器自己一顆 NT | silo（Windows 的 namespace 類物件）或 一容器一台小 VM | 只有 Hyper-V isolation 才起 |

Windows 容器兩種模式，差在 **kernel 共不共用**，不在硬體資源：

| | process isolation | Hyper-V isolation |
|---|---|---|
| kernel | 共用 host NT | 每容器一台極小 utility VM、自己一顆 NT |
| 隔離強度 | 中 | 強 |
| 版本限制 | image 的 Windows 版本**必須**和 host kernel 相容（NT user/kernel ABI 不穩定） | 無，image 自帶 kernel |
| 成本 | 毫秒、幾 MB | 秒級、幾百 MB |

Windows 容器不需要 Linux、不跑在 WSL2 裡，是 NT 自己的 silo 機制；跟 Linux 容器 image 不互通。

一句話：要 Linux 的東西（Claude sandbox、Linux 容器）→ 一定要 Linux kernel → 一定在 WSL2 那一台 VM 裡各佔一個 distro；要 Windows 的東西（Windows 容器）→ 不需要 VM，除非要 VM 等級隔離或 kernel 版本對不上。

## 10. namespace 按子系統各一張表，不是整面牆

八種：mount、pid、net、uts、ipc、user、cgroup、time。某個東西有沒有被隔開要問兩件事：(a) 它屬於哪個子系統、那個子系統有被 namespace 化嗎？(b) 兩邊真的拿到**不同的 instance** 嗎？

| sysctl | 屬於 | 有 namespace 嗎 | WSL2 兩 distro | 結果 |
|---|---|---|---|---|
| `net.ipv4.ip_forward`、`net.core.somaxconn` | network | 有（per-netns） | **共用同一個 netns** | Debian 跟著變 |
| `kernel.pid_max`、`vm.swappiness`、`fs.file-max`、`vm.overcommit_memory` | kernel 全域 | **沒有** | — | 跟著變，任何容器都一樣 |
| `kernel.hostname` | uts | 有 | 各自一個 | 不跟著變 |
| `/proc/sys/fs/mqueue/*` | ipc | 有 | 各自一個 | 不跟著變 |

- 一台 VM 一顆 kernel，兩個 distro 的 `sysctl` 寫的是同一顆 kernel 的記憶體；沒有「distro 之下的 kernel」。
- 沒被 namespace 化的 kernel 狀態（全域 sysctl、kernel 模組、時間、page cache）在所有容器 / distro 間永遠共用。
- cgroup 是**限制**不是**視圖**：`/proc/meminfo` 在所有容器裡都是整台的數字。
- Docker 預設給每個容器自己的 netns；WSL2 刻意讓 distro 共用。

## 11. VM 生命週期：process 數決定，不是「utility VM 常駐」

```
t0  你在 Ubuntu            nohup server &             server 在 Ubuntu 的 pid namespace
t1  你關終端               bash exit                  pty 關；wslhost.exe 退出
t2  /init (VM, user mode)  數 Ubuntu pid ns 裡「除 distro init 外」的 process → 1 → distro 繼續
t3  ……server 一直跑……      VM 不關，因為還有 distro 活著
t4  server 崩潰 / 被 kill   process 數歸零 → /init 標 Ubuntu stopped
t5  /init                  所有 distro 都 stopped → 8 秒倒數
t6  /init → wslservice     倒數到 → HCS 關 VM → 記憶體全還 Windows
```

- 「誰在數」：VM 裡 user mode 的 `/init`，看每個 distro 的 pid namespace。VM 活著是結果不是原因。
- server 活多久 = 到它自己退出、`wsl --shutdown`、或 **Windows 登出 / 重開機**（VM 綁使用者 session；開機自啟要靠工作排程器或 `[boot] command`）。
- Docker Desktop 讓 VM 常駐，是 `docker-desktop` distro 裡 dockerd 永遠有 process 在撐，不是 VM 自己常駐。

> **補強**（兩個 timer，不是一個）：上面 t5 的「8 秒」是 distro 那層的歷史數字——distro 的 pid namespace 空了之後等幾秒才把 distro 標 stopped。VM 那層另有一個 `.wslconfig` 的 `[wsl2] vmIdleTimeout`（毫秒，Windows 11 才有，預設 60000），是「所有 distro 都停了之後再等多久關 VM」。兩個 timer 疊起來才是「關掉最後一個終端到記憶體還給 Windows」的全程。推論：數 process 的到底是 VM 裡的 `/init` 還是 Windows 側的 `wslservice`（舊名 LxssManager），這份對話推的是 `/init`，但沒看 source；WSL 開源後兩邊的碼都在 `microsoft/WSL`，這題留在 checklist。

---

## 12. 思維鏈：腦筋急轉彎與答案

**Q1. 兩個 distro 共用 kernel、各自 namespace。Ubuntu 的 `ps aux` 看得到 Debian 的 process 嗎？`kill -9` 殺得到嗎？看不到的話，`/proc/meminfo` 為什麼兩邊一樣？**
看不到、殺不到：pid 是 namespaced，Debian 的 pid 在 Ubuntu 的 pid ns 裡沒有名字，`kill` 找不到目標。`/proc/meminfo` 一樣：記憶體**不是** namespace，`meminfo` 是全域數字；cgroup 只做限制不做視圖。藍圖模式 6 的「名字 → 物件」表：pid 有一張 per-ns 的表，記憶體沒有。

**Q2. `cargo build` 在 `/mnt/c/work/some-repo` vs `~/work`，rustc 每開一個 `.rs` 走哪條管線、誰的 token 檢查？**
`/mnt/c`：每個 open 走 9P → 跨 VM → Windows 側 Plan9 server 用你的 **Windows token** `CreateFile` → NT SRM 查 NTFS ACL；Linux 的 uid 在這裡幾乎無關（mode bits 是 metadata 模擬）。`~/work`：ext4.vhdx 上的 vnode，Linux kernel 的 DAC 看 **Linux uid**，Windows 從頭到尾不在場。同一個問題在兩個位置答案相反——誰管取決於檔案住在哪顆 kernel 的檔案系統裡。

**Q3. Linux kernel 0-day 提權，三層各自後果？**
VM：guest kernel 淪陷，Windows 不受影響（hypervisor 邊界，除非 hypervisor 本身有洞）。container：共用 kernel，kernel 淪陷 = host 與所有容器一起淪陷。sandbox-runtime：同上，整個 distro / VM 淪陷。§5 表「共用什麼」那一列直接給答案：只有 VM 不共用 kernel。

**Q4. `docker run`、`wsl -d`、`bwrap` 底下是同一組 kernel 原語嗎？差別在哪層？**
同一組：`clone` / `unshare` 的 namespace 旗子、cgroup、`pivot_root` / mount、seccomp、capabilities。差別在 **user space 誰組裝、組了多少**：Docker 全套 namespace + cgroup + overlayfs + image + 自己的 netns；distro 由 `/init` 組 mount + pid（+ uts / ipc）但刻意共用 netns、無 image 層；bwrap 每次挑一個子集、無 image、預設不碰 cgroup。「容器 / distro / sandbox」是組裝方式的名字，kernel 不認識這三個詞。

**Q5. 開了 WSL2 後 Windows 自己變 root partition。Windows kernel 跑在哪個 CPU 模式？跟 guest 的差別是模式不同還是權限不同？**
同樣是 VMX non-root（guest 模式）。差別是 hypervisor 給 root partition 的**權限**：直接存取實體裝置、能發管理其他 partition 的 hypercall、記憶體對映接近 1:1。模式相同、權限不同——跟 note 05「同 uid 不同 label」是同一種形狀。

**Q6. KVM 下 guest `top` 的 100% 和 host `top` 看 qemu process 的 CPU%，誰包含誰？VM exit 的時間算哪邊？**
host 看到的包含 guest 看到的：qemu process 的 CPU% = guest 原生執行時間 + VM exit 處理 + 裝置模擬 thread。VM exit 那段對 guest 不可見（guest 的 vCPU 被暫停），只會以 `st`（steal time）出現在 guest 的 `top`，前提是 hypervisor 有回報。guest 的 100% ⊂ host 的 qemu%。

**Q7. GCP live migration 搬的是什麼？對「VM = process」是補強還是反例？**
搬 VM 記憶體頁（pre-copy 反覆複製髒頁）、vCPU 暫存器、虛擬裝置狀態，最後一刻切換，目的端重建 EPT 與 qemu / VMM process。補強：VM 確實是一個可序列化狀態的 process。還多一層：VM **比一般 process 更好搬**，因為它對外的介面只有幾個虛擬裝置，不像普通 process 手上有一堆綁死在 host 的 fd / socket / pid。

**Q8. Windows 有 silo 這種 namespace 類機制，為什麼 Claude Code 在 native Windows 還是沒 sandbox——缺的是隔離原語還是別的？**
不是缺原語，是缺「**對任意子 process、每次呼叫、不需管理員、路徑白名單式**」的現成形狀。silo 要 Docker service + image + 多半要 admin；要自己組得用 token + job + ACL 副作用或 broker + WFP（identity-based），政策判定一部分回到 user mode。WSL2 一行不用改。見 note 05 §10 的路 A / 路 B。

**Q9. Ubuntu 裡 `sysctl -w net.ipv4.ip_forward=1`，Debian 跟著變嗎？**
會。兩個理由疊加：WSL2 distro 共用同一個 network namespace；就算分開了，`vm.swappiness` 這類全域 knob 根本沒有 namespace 版本。見 §10。

**Q10. `nohup server &` 後關掉所有終端，server 活多久？誰在數？**
活到 server 自己退出、`wsl --shutdown` 或 Windows 登出 / 重開。數的是 VM 裡 user mode 的 `/init`，數的是該 distro pid namespace 裡除 init 外的 process；歸零才停 distro，所有 distro 停才倒數關 VM。VM 活著是結果不是原因。見 §11（含「兩個 timer」補強）。

**Q11. `docker run --network host` 在 WSL2 上，容器進了誰的 netns？**
dockerd 所在的 netns = `docker-desktop` distro 的 netns = 因為 WSL2 所有 distro 共用一個 netns = 整台 VM 的 netns（Ubuntu 也在裡面）。不是 Windows 的。Windows 透過 localhost forwarding 或 mirrored 模式才碰得到它。

> **補強 Q12**（checklist 留的題：WSL2 裡 `sudo cat /mnt/c/Windows/System32/config/SAM`，Linux root 跳過 DAC，會成功嗎？死在哪一行、用誰的身分？）
> 不會。Linux 這邊 root 確實跳過 DAC，drvfs 的 mode bits 又只是 metadata，所以 Linux 側一路放行，`open()` 變成一個 9P `Tlopen` 跨 VM 送到 Windows 側的 Plan9 server（§4）。那個 server 用的是**啟動 WSL 的那個 Windows 使用者的 token**，不是 SYSTEM：`NtCreateFile` → `SeAccessCheck` 查 SAM 的 DACL（只給 SYSTEM 與 Administrators；就算你是 admin，UAC 沒提升的 token 裡 Administrators 是 deny-only SID）→ `ERROR_ACCESS_DENIED`。就算用提升過的 token 過了 ACL，SAM hive 被 kernel 以獨佔方式開著，還會撞 sharing violation。錯誤經 9P 回到 Linux 變成 `EACCES`（推論：drvfs 把 Windows 的 access denied 對到 EACCES，未實測；也可能是 EPERM）。一句話：死在 Windows 側的 `SeAccessCheck`，身分是你的 Windows token；Linux 的 root 在那條線上沒有任何意義——跟 Q2「誰管取決於檔案住在哪顆 kernel 的檔案系統裡」是同一個答案。整理者推的，待 /校正。

---

## 13. 陷阱清單（誤解修正記錄）

標【模型】是概念錯、會導致錯誤預測；標【用詞】是說法不精確。

1. 【模型】❌ Docker 是 Windows 上另一種 sandbox 手段 → ✅ Docker Desktop 的 Linux 容器自己住在 WSL2 VM 的 `docker-desktop` distro 裡，是同一台 VM 的房客 → 為什麼：Linux 容器需要 Linux kernel，Windows 上唯一那顆 Linux kernel 就在 WSL2 utility VM 裡；它是可疊加的第二層（§5），不是第三層的替代品。

2. 【模型】❌ WSL2 的 VM 不是雲上那種 VM → ✅ 同一種東西（hypervisor 切出來的 child partition），只是拿掉 UEFI 與模擬裝置、kernel 直接載進記憶體 → 為什麼：Hyper-V 對 utility VM 與 Azure VM 做的是同一件事（vCPU 排程、EPT、VMBus）；差的是啟動流程與裝置清單，不是隔離機制（§7–§8）。

3. 【模型】❌ hypervisor 用記憶體資源「模擬」Linux kernel → ✅ 不模擬：guest 指令原生跑在 CPU 上，hypervisor 用 VT-x / EPT 切硬體、只在特權動作時介入 → 為什麼：模擬（逐條翻譯）只在 ISA 不同時需要；WSL2 的 x86-64 kernel 跑在 x86-64 上零翻譯，慢的只有 VM exit 那幾步（§6）。

4. 【模型】❌ sandbox-runtime 是 Linux kernel 的 sys → ✅ Anthropic 的 user space npm 套件，呼叫 bwrap 與 seccomp → 為什麼：跟 Seatbelt 線同構——harness 出題、kernel 機制執行（note 05 §3）；它自己一個 syscall 都擋不了，擋的是 namespace 與 seccomp。

5. 【模型】❌ Hyper-V 之前靠 kernel 分配、之後變主從 → ✅ 之前沒有 VM 這回事；root / child 是 type-1 的架構設計 → 為什麼：hypervisor 太小不含 driver，必須指定一個特權 partition 替它管實體裝置；Xen 的 dom0、KVM 由 host kernel 兼任，都是同一個答案（§7）。

6. 【模型】❌ 一台實體機 partition 多個 hypervisor 給客戶 → ✅ 一個 hypervisor 切多台 VM；客戶拿到 VM → 為什麼：hypervisor 是那台機器最底層的仲裁者，只能有一個；要再疊一層是 nested virtualization，VMX 本身要被虛擬化（§8）。

7. 【模型】❌ distro 是某個 VM → ✅ distro 是一台 VM 裡的容器（namespace + 自己的 vhdx）；所有 distro 同一顆 kernel → 為什麼：`wsl -d` 只是請 `/init` 建一組 namespace 掛 vhdx；`wsl --shutdown` 全部一起死、`sysctl` 互相影響，都是「一顆 kernel」的直接後果（§1、§10）。

8. 【模型】❌ utility VM 是常態 VM → ✅ lazy start + idle shutdown → 為什麼：VM 活著是「還有 distro 有 process」的結果；Docker Desktop 讓它常駐是 dockerd 一直有 process，不是 VM 自己常駐（§11）。

9. 【模型】❌ Windows 容器要不要 VM 的差異在硬體資源 → ✅ 差在 kernel 共不共用 → 為什麼：process isolation 共用 host NT，所以 image 版本必須與 host kernel 相容、kernel 漏洞全體淪陷；Hyper-V isolation 每容器一顆 NT，付的是秒級與幾百 MB（§9）。

10. 【模型】❌ sysctl 改在「distro 之下的 kernel」，不同 namespace 鐵定不影響 → ✅ 一顆 kernel；namespace 按子系統各一張表 → 為什麼：某個 knob 被不被隔開要看它屬於哪個子系統、那個子系統有沒有 namespace 化、兩邊是否拿到不同 instance；WSL2 distro 共用 netns，`vm.*` 這類全域 knob 根本沒有 namespace 版本（§10）。

11. 【模型】❌ server 一直跑是因為 utility VM 常駐 → ✅ 因為該 distro 的 pid namespace 裡還有 process → 為什麼：因果反了——有 process 才有 distro、有 distro 才有 VM；VM 不會為了「還可能有人要用」而活著（§11）。
