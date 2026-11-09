# OS 學習方向 checklist

> [`os.md`](os.md) 是地圖，這份是進度表：六個子系統各挖到哪裡、下一條線挖什麼、還有哪些題沒答。
> 每份 note 或經驗入庫後回來做三件事：覆蓋表「已挖到」補 note 編號、把對應項目從「還沒碰」移走；待挖路線打勾並記 note 編號；未答的題有答案的打勾並註記出處。
> 建立日期：2026-11-05｜最近更新：2026-11-09

---

## 1. 覆蓋表：六個子系統各挖到哪裡

| 子系統 | 已挖到 | 還沒碰的核心 |
|---|---|---|
| ① process | thread 本質、scheduler、wait queue（02）；fork / exec / spawn、ucred、process tree（05）；每個欄位各自複製／共用／重設、clone 與 wrapper（05 §11） | process 生命週期（zombie / wait / SIGCHLD）、process group / session / controlling tty / job control；boot 到 pid 1 |
| ② 記憶體 | stack / heap（01）、COW 提到（05）、ELF 載入（04）、page cache 與 fsync（02）；動態載入器流程 PT_INTERP → NEEDED → 符號版本 → 重定位 → `_start`、`dlopen` 與 NSS（經驗 01 §7） | **page fault 完整時間軸**、頁表與 TLB、mmap / brk 與 malloc、swap——六個裡最缺的一塊 |
| ③ 檔案 | fd / fileglob / vnode / VFS（05）；flock 掛 fileglob、git `.lock` 的 `O_EXCL + rename`（05 §9）；soft link 在 `namei` 解掉、MAC 看的是解析後路徑，`/var → /private/var`（經驗 02 §4–§5） | inode 是什麼、目錄只是 name→inode 表、hard link、unlink 只刪名字、mount 怎麼接樹 |
| ④ IPC | epoll（01 / 02）、SCM_RIGHTS（05）、Mach port 與 right（05 §13）；unix socket 是有路徑的 vnode、`connect(AF_UNIX)` 走 VFS，ssh-agent 的簽名協議（經驗 02 §3–§4） | IPC 全家福對照（pipe / FIFO / signal / shm / Mach port / futex）；網路 stack 一次沒碰 |
| ⑤ 安全 | DAC / MAC / Seatbelt / capability（05）；Windows token / ACL / AppContainer 與 WSL2 對照（05 §10）；`network-outbound` 單一 proxy 出口、unix-socket filter 字面比對、kernel 層 vs harness 分類器（經驗 02 §4、§6） | Linux 四根柱子（namespace / cgroup / seccomp / LSM）→ 容器；隔離三層級 |
| ⑥ 裝置 | driver 兩半、ioctl、DMA、中斷（02 / 03） | block layer 與 page cache 的接點 |

## 2. 待挖路線（每條一份 note）

排序：1、2 先做——沒有 page fault 模型，COW / mmap / page cache 只能背；沒有 session / tty 模型，launchd / daemon / sudo 只能記特例。

- [ ] **1. page fault 的一生**
  - 切入點：`vec![0u8; 1<<30]` 後看 RSS 為何接近 0，寫一下才漲（`vmmap` / `/proc/self/status`）
  - 補哪裡：② + 模式 2、7；COW、mmap 的前置
- [ ] **2. process 的生死：zombie、SIGCHLD、session、tty、job control**
  - 切入點：`sleep 100 &` → `ps -o pid,ppid,pgid,sid,tty,stat`；Ctrl-Z / `fg` 誰收到什麼 signal
  - 補哪裡：① + 模式 3；解釋 sudo 沒 tty、daemon 為何 `setsid`
- [ ] **3. 從按電源到 shell：firmware → bootloader → kernel → launchd / systemd → login → zsh**
  - 切入點：`launchctl list` / `systemctl status`；從 pid 1 畫樹
  - 補哪裡：① + 模式 3
- [ ] **4. inode 與名字：hard / soft link、unlink 已開檔案、mount**
  - 切入點：`ln`、`stat -f %i`、open 後 `rm` 再 `cat /dev/fd/N`
  - 補哪裡：③ + 模式 1、6
- [ ] **5. IPC 全家福**
  - 切入點：兩支 Rust 程式用三種方式傳同一筆資料，`strace` / `dtruss` 看 syscall
  - 補哪裡：④ + 模式 1、3
- [ ] **6. Linux 容器四根柱子，對照 Seatbelt**
  - 切入點：`unshare -m -p -f`、`bwrap`、`grep Seccomp /proc/self/status`
  - 補哪裡：⑤ + 模式 5、6；接 Claude Code Linux sandbox
- [ ] **7. 隔離三層級：sandbox / container / VM**
  - 切入點：同一個 `uname -r` 在 macOS Docker、WSL2、原生各看到誰的 kernel
  - 補哪裡：⑤ + 模式 6；收 WSL2 支線
- [ ] **8. 網路 stack：socket → TCP → IP → NIC → 中斷 → epoll**
  - 切入點：接 note 02 的 `socket.read().await`，往下補 NIC 到 socket buffer
  - 補哪裡：④ + ⑥
- [ ] **9. 動態連結與載入器：`ld.so` 從 PT_INTERP 到 `_start`**
  - 切入點：`readelf -l / -d`、`LD_DEBUG=libs ./app`、`LD_PRELOAD` 換掉 `malloc` 看現象
  - 補哪裡：② + 模式 4、6；接 note 04 ELF 的下半段（section 之後、process 之前），經驗 01 §7 只走了主幹

## 3. 未答的題（腦筋急轉彎）

這些是各條線留下、還沒用新模型推過的題；答了就打勾並註記在哪份 note / 經驗。入庫時整理者推的答案算「補強」，/校正 通過才打勾。

**Windows / WSL2**

- [ ] Windows：integrity label 與 DACL 都回 `ERROR_ACCESS_DENIED`，不真的開檔怎麼分辨哪道擋的？（提示：`AccessCheck()`）
- [ ] 路 A（ACL）與路 B（broker）各自撞到 Claude Code「每次呼叫獨立產一張、可單次關閉」的設計什麼？哪個更接近「process 死 policy 就消失」？
- [ ] WSL2 裡 `sudo` 讀 `/mnt/c/Windows/System32/config/SAM`：Linux root 跳過 DAC，會成功嗎？死在哪一行、用誰的身分？

**Linux sandbox 對照**

- [ ] bubblewrap（ENOENT）與 Seatbelt（EPERM）下，同一個 git 讀 `~/.gitconfig` 行為一樣嗎？

**git 與鎖**

- [ ] `O_CREAT|O_EXCL` 的原子性誰保證？`.git` 在 `/mnt/c`（9P）上還成立嗎？
- [ ] git `.lock` 是 advisory，agent 直接 `echo sha > .git/refs/heads/main` 照樣寫入；sandbox 為何擋不了？要擋得寫在哪層、用什麼 filter？
- [ ] `posix_spawn` + `POSIX_SPAWN_CLOEXEC_DEFAULT` 下 C 還持有鎖嗎？A 的 Bash 被 SIGKILL，鎖誰釋放、在哪條 thread？

**fd 與 CLOEXEC**

- [ ] 多 thread 程式 `open` 與 `fcntl(FD_CLOEXEC)` 兩步之間另一條 thread fork+exec，fd 洩漏了嗎？`O_CLOEXEC` 解決什麼？
- [ ] zsh 若沒在自己的 fd 上設 CLOEXEC，git 會看到什麼？一個 fd 洩漏進 sandboxed 子 process，Seatbelt 擋得了它 read 嗎？

**Mach**

- [ ] fd table 繼承在 port 這邊的對應是什麼？fork 後不 exec 為何 undefined——哪種 right 複製了會出事？launchd 持有 daemon 信箱的 receive 還是 send right？client 要 `mach-register` 自己得交出什麼？

**sandbox 與 socket**（經驗 02 留下）

- [ ] Seatbelt 的 `remote unix-socket` filter 拿到的是 `connect()` 傳入的字串還是 `namei` 解析後的路徑？經驗 02 §4 觀察到 symlink 不命中，但沒看 Sandbox.kext 在哪個 hook 取路徑；`file-read*` 對 symlink 本身的檢查又是在哪一步？
- [ ] `allowUnixSockets` 的 glob 不生效是 harness 翻成 `literal` 還是 Seatbelt 的 `regex` filter 對 unix-socket 不適用？（經驗 02 §4 推論，待查 harness 產出的 SBPL）
- [ ] 一條 HTTP `CONNECT` 建好後 proxy 變成純 TCP 管子：proxy 還能看到什麼？`allowedDomains` 對 CONNECT 的 host 檢查發生在哪一刻，隧道內的 SSH 握手它看得到嗎？
- [ ] ssh-agent 的 socket 權限 `srw-------` 是 DAC；同 uid 的任何 process 都連得上。Seatbelt 之外，macOS 有什麼機制能限制「哪個 process 可以請 agent 簽名」？（提示：agent 看得到 client 的 pid 嗎、`LOCAL_PEERCRED`）

**libc / ABI / 載入器**（經驗 01 留下；經驗 01 §11 有入庫時推的答案，待 /校正）

- [ ] 靜態 musl 的 Rust binary 在 macOS 能跑嗎？死在 kernel、載入器、還是 libc？
- [x] 靜態 musl binary 沒有 NEEDED、沒有 PT_INTERP，還有「ABI 依賴」嗎？依賴三層契約的哪一層？→ 經驗 01 §6、§9：只剩 ISA 與 kernel syscall ABI，再加資料檔與環境依賴
- [ ] glibc 2.34 把 `libpthread` 併進 `libc.so.6`：2020 編的 binary 在 2024 Debian 上怎麼還能跑（靠哪個機制）？反過來 2024 編的放 2020 Debian，死在哪一行、誰印錯誤？
- [ ] Docker 在 macOS 上跑 ELF + glibc 的 Linux image，按五層檢查應死在 ②；Docker Desktop 在哪一層動了手腳？
- [ ] 同一台 Linux，2012 年編的靜態 binary 與 2012 年編的 glibc 動態 binary，哪個今天更可能還能跑？各依賴哪一層的承諾？
- [ ] FreeBSD 的 Linux 相容層補的是 ③，那 ④⑤ 的 `ld-linux` 和 `libc.so.6` 從哪來？跟 WSL1 是同一種做法嗎？
- [ ] 2024 Ubuntu 編的 Rust binary（gnu target）放 2020 Ubuntu 死在 ⑤。不換 target、不改機器，只改 build 設定能不能過？（提示：⑤ 錯誤訊息裡的版本號是誰決定的。）
- [ ] Linux 和 macOS 都是 x86-64、System V 呼叫慣例，把 Linux 的 `.o` 拿去 macOS 連結，差在哪一層？（提示：符號名前的底線、note 04 的 section 名。）
- [ ] Rust 沒有穩定 ABI，但 `std` 是預編好的。矛盾嗎？cargo 靠什麼保證你的 crate 和預編的 `std` 擺法一致？
- [ ] 假設 glibc 改了 `struct stat` 欄位順序，但 syscall 號碼、符號名、版本標籤都沒動。舊 binary 能啟動嗎？何時出事、長什麼樣？「有標籤的檢查」保護了多少？
