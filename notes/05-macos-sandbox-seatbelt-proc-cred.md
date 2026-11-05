# Note 5 — macOS Sandbox（Seatbelt）：從一條被擋的 `git` 挖到 proc、cred 與 MAC

> 這份筆記記錄的是「為什麼 Claude Code 的 Bash 有時能跑、有時 `Operation not permitted`」這條線：從 `sandbox-exec` 與 SBPL 起步，穿過 kernel 的 MAC framework、process 的 credential 與 label、fork / exec / posix_spawn 的繼承規則、fd table 與 kernel 全域 file object、XPC 代辦與 `sandbox_check`，最後落到 DAC / MAC / VFS 三者的關係。
> 核心收穫不是 Seatbelt 的語法，而是四句話：**sandbox 不是一個模式，是 process credential 上的一個欄位**；它跟著 fork / exec 的血緣走，不跟著 IPC 的請求走；檢查發生在「取得資源」的 syscall，不在「使用資源」的 syscall；kernel 是唯一看得到所有 process、又站在所有 syscall 路徑上的東西。
>
> 記錄日期：2026-11-04｜當時定位：🥚 小白（仍未動手寫程式），這條線是 OS / systems 支線，碰到 🐊 的 kernel 資料結構（`proc` / `ucred` / `fileglob`）、MAC framework 與 Mach IPC。Claude Code 這個產品本身的 sandbox 設計另記於 [rookie-llm Case 02](https://github.com/LouisSun1994/rookie-llm/blob/main/cases/02-claude-code-sandbox-seatbelt-design.md)，此處只記 OS 概念。
> 2026-11-05 續：§9–§13、思維鏈 E–H、陷阱 23–37 來自這份 note 產出後的追問——從「Windows 上怎麼做」挖到多 agent 競爭與 flock、fork / exec 每個欄位的繼承真相、Linux 的 clone、硬體與 kernel 的分工、Mach。是同一條線的延伸、經導師校正，不是入庫加註。追問裡整理出的 OS 藍圖（家譜、六個子系統、七個模式）另放 [`blueprints/os.md`](../blueprints/os.md)。
> Seatbelt 是 Apple 的 private API，operation / filter 清單來自逆向工程（Levin《*OS Internals》、Blazakis〈The Apple Sandbox〉）與 `/System/Library/Sandbox/Profiles/*.sb`，各版 macOS 有增減。
> 標「補強」「補充」的 blockquote 是整理進 repo 時加註的延伸，不在原本這條線的對話裡。

---

## 0. 一句話版本

Claude Code 每次開 shell 前產一張 default-deny 的 syscall 白名單（SBPL），交給 Apple 出貨的 `sandbox-exec` 編譯後塞進 kernel、掛在那個 process 的 `ucred.cr_label` 上；之後 shell 和它 fork / exec 出來的所有子孫，每個「取得資源」的 syscall（open、execve、connect、mach_lookup）都由 kernel 的 Sandbox.kext 拿這張標籤比對，不過就 `EPERM`。它是過濾不是隔離：同一個檔案系統、同一個 uid，只是某些 syscall 會失敗。

---

## 1. 分層模型（骨架）

```
user   Claude Code (Node)        組 SBPL 字串、spawn sandbox-exec；sandbox_init 之後完全不在迴路
user   /usr/bin/sandbox-exec     Apple 出貨的 CLI 殼：parse 參數 → 呼叫 libsandbox → execvp 你的程式
user   libsandbox.dylib          內建 TinyScheme「執行」SBPL 產出 bytecode；包 sandbox_init / sandbox_check / sandbox_extension_*
user   XPC services              cfprefsd、launchservicesd、opendirectoryd…：process tree 另一條分支上的 daemon
═══════ syscall 邊界（trap）═══════
kernel MAC framework             TrustedBSD hooks，橫向插在 VFS / network / Mach IPC / process / sysctl 各子系統
kernel Sandbox.kext              policy module：讀 ucred.cr_label 那格，NULL → allow，否則跑 bytecode
kernel VFS                       namei → vnode → [MAC hook] → [DAC: vnode_authorize] → VNOP_*
kernel proc / ucred / fileglob   資料結構：label 住 ucred；fd 是 proc 的 table，指向全域有 refcount 的 fileglob
kernel launchd 視角              pid 1 是所有 daemon / agent / GUI app 的 parent；它沒有 label
kernel Mach                      task / thread / vm_map / port：每個 proc 包一個 task；port right 是 IPC 的 capability（§13）
═══════ 硬體邊界 ═══════
硬體   CPU / MMU                  特權等級（EL0 / EL1）、trap 指令、exception vector、page table：只給門和隔間，不知道 pid / fd / label（§12）
```

每個概念屬於哪一層：

| 概念 | 層 |
|---|---|
| SBPL 文字、`(deny default)`、`(allow file-read* (subpath …))` | user：policy 的 source |
| bytecode（operation → filter → verdict 的決策圖） | user 產、kernel 持有 |
| `sandbox_init()`、`sandbox_check()`、`sandbox_extension_*` | libsandbox（user），最後一步是 `__mac_syscall` |
| `ucred.cr_uid`、`ucred.cr_label`、`proc.p_fd`、`fileglob.f_cred` | kernel 資料結構 |
| `mac_vnode_check_open`、`vnode_authorize` | kernel VFS 內的兩個 hook |
| `EPERM` / `EACCES` / `ENOENT` | kernel 回給 user 的 errno，各對應不同檢查點 |
| zsh、git、sudo、launchctl、osascript | user process，各自帶一張 ucred |
| cfprefsd、launchservicesd | user daemon，由 launchd spawn，不在你的繼承子樹上 |
| Edit / Read / Write / MCP 工具 | Claude Code 自己的 process 或獨立 process，**不經 sandbox-exec** |
| task、thread、vm_map、port right | kernel：Mach 層，BSD 的 proc / fd / signal 架在它上面（§13） |
| `svc #0x80`、EL0 / EL1、exception vector、page table | 硬體：ISA + 特權模式 + MMU，沒有 API（§12） |
| `clone()`、namespace、seccomp、bubblewrap | Linux kernel：同一個位置的不同實作（§10–§11） |
| access token、Job object、AppContainer SID、ACL | Windows NT kernel：identity-based 的對應物（§10） |

---

## 2. Seatbelt 是什麼：kernel 層的 MAC，不是容器

**核心規則**：macOS kernel 的 TrustedBSD MAC framework 在每個 syscall 進 kernel 時多做一次策略檢查；Sandbox.kext 是掛在上面的 policy module，策略用 SBPL 描述。檢查的對象是 **syscall 類別 + 資源**，不是「指令」。

**關鍵細節**
- 它是過濾器（filter），不是隔離（isolation）：沒有 namespace、沒有另一個 rootfs、uid 相同。被擋只是 syscall 回 `EPERM`，不是「看不到」
- 「reachable ≠ provided」：技術上把 `/Users/admin` 加進白名單就能讀到 `~/.ssh`，所以 sandbox 的邊界同時是「這個 task 被授權範圍」的宣告
- 判定的不是「合法」——kernel 沒有合法的概念，只會拿 profile 逐條比對。「安全性」來自那張 least-privilege 白名單，kernel 只負責強制執行

**對照表**

| | Seatbelt | Linux seccomp-bpf | AppArmor | Docker |
|---|---|---|---|---|
| 層 | kernel MAC policy | kernel syscall filter | kernel MAC（path-based） | namespace + cgroup + 上述組合 |
| 看什麼 | syscall 類別 + 路徑 / mach name / IP | syscall 號碼 + 暫存器值 | 路徑 + capability | 另一個世界 |
| 另一個 rootfs | 無 | 無 | 無 | 有 |
| uid | 相同 | 相同 | 相同 | 可 remap |
| 最接近的類比 | AppArmor + seccomp | — | — | 不同層級 |

---

## 3. SBPL 與 `sandbox-exec`：出題和閱卷分開

**核心規則**：SBPL（Sandbox Profile Language）是 TinyScheme 方言的 **policy 描述語言**——寫的不是「要做什麼」，是「哪些 operation 對哪些資源 allow / deny」。`sandbox-exec` 不是 runtime，是 **loader**：編譯、塞進 kernel、exec 你的程式，然後它自己被 exec 取代而消失。真正跑 bytecode 的引擎一直在 kernel 裡。

**關鍵細節**

```scheme
(version 1)
(deny default)                                   ; 預設全 deny，再開洞
(import "system.sb")                             ; 引入系統共用片段
(define repo (param "REPO"))                     ; 從 sandbox-exec -D REPO=... 拿參數
(allow process-exec* process-fork)
(allow file-read* (subpath "/usr") (subpath "/System"))
(allow file-read* file-write* (subpath repo))
(deny  file-read* (literal (string-append repo "/.env")))   ; 後面的規則蓋前面
(allow network-outbound (remote ip "localhost:7777"))
(deny  file-read* (subpath "/Users/admin/.ssh") (with report))  ; 被擋時寫 log
(debug deny)                                     ; 所有 deny 都寫進 unified log
```

- 規則語法固定是 `(<verdict> <operation>... <filter>... [modifier])`
- 同一個 operation 多條規則命中時 **後宣告的贏**；Claude Code 把長路徑排後面，得到 longest-prefix-match 的效果
- operation 名字支援 `*` 結尾當 family wildcard（`file-write*` 涵蓋所有 `file-write-xxx`）；沒被提到的 sub-op 往上找 family，再找 `default`
- 讀和寫是分開的 operation family（`file-read*` vs `file-write*`），所以「讀得到但寫不進」很正常
- SBPL 真的被當 Scheme 程式跑：libsandbox 內建 TinyScheme 直譯器，執行的副作用是產出 bytecode。所以 `(define …)`、`(string-append …)`、`(param …)` 都能用

**Operation families（逆向整理，常見的）**

```
family                 sub-operations                                   對應的 kernel 入口
─────────────────────────────────────────────────────────────────────────────────────────
default                                                                  兜底
file-read*             -data -metadata -xattr                           open / stat / getxattr
file-write*            -data -create -unlink -mode -owner -flags -times  open(O_WRONLY) / creat / unlink /
                       -xattr -setugid -mount -unmount                   chmod / chown
file-ioctl file-search file-link file-chroot file-map-executable file-issue-extension
process-exec*          -interpreter                                     execve（shebang 另算）
process-fork                                                             fork / posix_spawn
process-info*          -pidinfo -listpids -codesignature …              proc_info / sysctl KERN_PROC
network-outbound  network-inbound  network-bind                         connect / accept / bind（含 unix socket）
mach-lookup  mach-register  mach-bootstrap  mach-priv-host-port          bootstrap_look_up（XPC / 所有系統服務的門口）
ipc-posix-shm*  ipc-posix-sem  ipc-sysv-*                                shm_open / semaphore
signal                                                                   kill
sysctl-read / -write                                                     sysctl
system-socket system-fsctl system-kext-load system-privilege system-info system-sched system-set-time
iokit-open  iokit-get-properties  iokit-set-properties                  IOServiceOpen（驅動）
appleevent-send                                                          AESend（osascript 卡這裡）
lsopen                                                                   Launch Services（open 指令）
distributed-notification-post  user-preference-read / -write  pseudo-tty  job-creation  nvram*  hid-control
```

**Filters（每個 family 只接受自己那類）**

| 類別 | filter | 例子 |
|---|---|---|
| 路徑 | `literal` `subpath` `prefix` `regex` `path-ancestors` `home-*` `vnode-type` `file-mode` `xattr` `extension` | `(subpath "/Users/admin/work")` |
| 網路 | `local ip` `remote ip` `remote tcp` `remote udp` `local unix-socket` `remote unix-socket` `socket-domain` `socket-type` | `(remote ip "localhost:7777")` |
| mach | `global-name` `global-name-prefix` `global-name-regex` `local-name` | `(global-name "com.apple.cfprefsd.agent")` |
| sysctl | `sysctl-name` `sysctl-name-prefix` `sysctl-name-regex` | `(sysctl-name "kern.osversion")` |
| iokit | `iokit-user-client-class` `iokit-registry-entry-class` `iokit-property` | |
| process | `target self\|pgrp\|children\|others\|same-sandbox` `process-attribute` `require-entitlement` `require-code-signature` | signal 給誰 |
| appleevent | `appleevent-destination` | `"com.apple.Terminal"` |
| 組合 | `require-all` `require-any` `require-not` | |

Modifier 接最後，改「判定之後做什麼」：`(with report)`、`(with no-log)`、`(with send-signal SIGKILL)`、`(with message "…")`。

**libsandbox 的 C API**

```c
sandbox_init(profile, flags, &err)                                 // 編譯 + 塞 kernel；一次性、只能收緊
sandbox_init_with_parameters(profile, flags, params[], &err)       // 對應 sandbox-exec -D
sandbox_check(pid, "file-read-data", SANDBOX_FILTER_PATH, path)    // 問 kernel「這個 pid 能不能」，不真的做
sandbox_extension_issue_file(class, path, flags)                   // sandbox 外的 process 發一張 token
sandbox_extension_consume(token)                                   // sandbox 內的 process 吃下 → 動態放寬（唯一的放寬路）
sandbox_extension_release(handle)
```

**公開程度三層（回答「這是不是瞎子摸象」）**

| 層 | 狀態 | Claude Code 用到？ |
|---|---|---|
| App Sandbox entitlements | 公開、有文件、Apple 支援 | 否（要 code sign、粗粒度、不能動態套給任意子 process） |
| `sandbox-exec` CLI / `sandbox_init`（SDK 有 `sandbox.h`） | 有出貨、有 man page，10.8 起標 deprecated，14 年沒拿掉（Bazel、Nix、Homebrew build、Chromium 都靠它；App Sandbox 底層也是同一套） | 是，這是介接點 |
| 完整 SBPL 語法、operation 清單、`sandbox_extension_*` | 未文件化，逆向 + 讀 Apple 自己的 `.sb` | 寫 profile 需要這些知識 |

所以「瞎子摸象」只對一半：接線的路是 Apple 提供的，寫規則的知識才是逆向的；而且機器上有 Apple 自己寫的 `.sb` 當語法範本、編譯器對不認識的 operation 直接報錯，等於有 reference implementation。Apple 每版新增 operation / service 時，`(deny default)` 讓不認識的東西一律 deny → interface drift 的後果是「功能壞」不是「安全破洞」（fail-closed）。

---

## 4. label：ucred 上的第二個 uid

**核心規則**：label 是 MAC framework 的通用詞——掛在 kernel 物件上、給 policy module 自用的標籤。Seatbelt 用的是 **process credential 上那一格**。「有沒有 label」字面上就是那格是 NULL 還是指向一張 bytecode。

**關鍵細節**

```c
struct proc {
    pid_t          p_pid;
    struct ucred  *p_ucred;  ──▶  struct ucred {
    struct filedesc *p_fd;           uid_t          cr_uid;        // DAC（rwx）看這格
    ...                              gid_t          cr_groups[];
}                                    struct label  *cr_label;  ──▶  struct label {
                                 }                                     void *slot[N];   // 每個 MAC policy module 一格
                                                                   }
                                                                   Sandbox.kext 那格 ──▶ bytecode（沒 sandbox 的 process：NULL）
```

> **補充**：上圖是簡化版。現行 XNU 的 `struct ucred` 把 POSIX 身分（`cr_uid` / `cr_ruid` / `cr_svuid` / `cr_groups[]`…）收在子結構 `cr_posix` 裡，`cr_label` 則直接掛在 `ucred` 上，kernel 取 uid 走 `kauth_cred_getuid(cred)`。分層結論不變：uid 與 label 是同一張 cred 上兩個獨立的軸。

- 每個 MAC hook 的第一個參數就是 credential：`mac_vnode_check_open(cred, vp, mode)`。kernel 在 syscall 入口拿 **正在 trap 進來的那個 process** 的 `p_ucred` 傳給 Sandbox.kext；它從頭到尾只看一個東西：發這個 syscall 的 process 自己的欄位。沒有「查一下對方是誰」「查一下這個請求最初是誰發起的」
- `sandbox_init` 只能收緊，沒有放寬的 syscall；唯一的放寬是 `sandbox_extension`（不改 label，給一張 capability）
- root（uid 0）對 label 沒有任何特殊地位：MAC 的 M 是 Mandatory，連 root 都不能選擇退出

**對照表：label 就是第二個 uid**

| | `cr_uid` | `cr_label`（Seatbelt） |
|---|---|---|
| fork | 複製給孩子 | 複製給孩子 |
| exec | 保留（setuid 例外：**可以升**） | 保留（無例外） |
| IPC 傳訊息給別人 | 對方的 uid 不變 | 對方的 label 不變 |
| process 自己改 | `setuid()`：非 root 只能降 | `sandbox_init()`：只能收緊 |
| kernel 依檔案屬性改 | setuid bit → 升到 owner | 無此機制 |
| 外部 process 遞東西進來 | 無 | `sandbox_extension`：不改 label，給 capability |

---

## 5. fork / execve / posix_spawn / pthread_create 與血緣

**核心規則**：fork / posix_spawn 建 **process**（附一條初始 thread）；execve **不建任何東西**，同一個 process 換腦；pthread_create 建 **thread**。label 掛在 ucred、ucred 是 process 的、thread 共用一張——thread 這一層沒有 label 這個概念。「在不在 sandbox 內」只問一句：**parent 鏈上有沒有經過 sandbox-exec**。

**關鍵細節**

```
呼叫               建新 proc?  地址空間          thread           ucred(uid+label)   fd table            之後跑的程式碼
──────────────────────────────────────────────────────────────────────────────────────────────────────────────
fork()             是          COW 複製父的       新 proc 裡 1 條   複製父的            複製父的             父的同一份，從 fork() 返回點繼續
execve()           否          丟掉，換新映像     只留 1 條         保留（setuid 例外） 保留（CLOEXEC 除外）  新執行檔，從 _start 開始
posix_spawn()      是          新的               1 條              複製父的            依 attr              新執行檔  ＝ fork+exec 一步完成
pthread_create()   否          共用               多 1 條           共用同一張          共用                 同一份，從指定函數開始
```

- fork 和 exec 拆成兩步的理由：中間那個「已經是新 process、但還跑著父的程式碼」的空窗，是 shell 做 `|`、`>`、`2>&1`、chdir 的地方。posix_spawn 合一是因為 fork 一個大地址空間很貴、在多 thread process 裡只複製呼叫的那條 thread，其他 thread 持有的 lock 永遠不會放
- 對到 Rust：`std::process::Command::spawn()` 在 macOS 走 posix_spawn；`Command::pre_exec()` 標 unsafe 就是因為它在那個空窗裡跑 closure；`std::thread::spawn()` 走 pthread_create
- `exec zsh` 不是 fork：同 pid、同 `struct proc`、同 `p_ucred`，只換記憶體映像，label 不動。`sandbox-exec` 自己也是這樣 exec 成 zsh 的，它從來沒有 fork
- setuid exec（`sudo`）：kernel 在 execve 看檔案 mode bit 的 `S_ISUID` + owner root，配一張新 ucred `euid=0, ruid=501`，`cr_label` 原封不動。root 只對 DAC 有意義。如果 setuid exec 會清 label，任何 sandbox 都能靠 exec 一支 setuid 程式逃出去
- launchd：macOS 10.10 起只有 **一個** launchd（pid 1），內部管多個 domain——system domain 以 root 跑 daemon，每個登入使用者一個 gui domain 以該 uid 跑 agent；GUI app 也是 launchd 直接 spawn 的（Launch Services 請它代開）。「daemon / agent」是 launchd 術語（有沒有綁 GUI session），不是 user 的名字

> **補充**：「`Command::spawn()` 在 macOS 走 posix_spawn」有條件——std 只在沒有 `pre_exec`、沒有要改 uid / gid 這類得在空窗裡做的事時才走 posix_spawn；一設 `pre_exec` 就退回 fork + exec，因為 posix_spawn 根本沒有「新 process 跑父程式碼」的那段空窗可以塞 closure。預測差別：同一支程式加了 `pre_exec` 之後，syscall trace 裡看到的是 fork + execve 而不是 posix_spawn。

**Process tree：同一支 git，一邊能跑一邊不能**

```
launchd (pid 1, root, label=NULL)
├── cfprefsd            uid root，system domain 的 daemon
├── cfprefsd            uid admin，gui domain 的 agent（同一支執行檔，第二個 instance）
├── Terminal.app        uid admin
│   └── zsh -l          uid admin，label=NULL
│       └── git         ✅ open ~/.gitconfig 正常
└── Claude.app (Node)   uid admin，label=NULL
    ├── [Edit 工具]     Node 自己呼叫 open()，label=NULL → kernel 放行；
    │                   「~/.claude/projects 不給寫」是 Claude Code 的 JS 在 app 層擋的
    └── sandbox-exec → zsh -lc '...'    ← label 從這個節點開始
        └── git         ❌ open ~/.gitconfig → EPERM，kernel 擋的
```

- 繼承是**血緣**不是**時間**：zsh 拿到 label 之後 launchd 才重新 spawn 的 cfprefsd 一樣乾淨，因為它的 parent 是 launchd
- `launchctl start job`：zsh 只寄一封信，真正呼叫 posix_spawn 的是 launchd，孩子的 ucred 複製自 launchd → label=NULL → 完整的 sandbox escape。所以 Seatbelt 必須在 `mach-lookup` 那一步堵 launchd 的管理 service（另有 `job-creation` operation）。**label 跟著 syscall 的執行者，不跟著意圖的發起者**
- zsh 是程式不是腳本：`/bin/zsh` 讀一行 → fork+exec → 等 → 下一行；`~/.zshrc` 才是腳本。Claude Code 每次 Bash 呼叫是一個全新的 `zsh -lc '<指令>'`，跑完就死；`-l` 讀 `~/.zprofile` / `~/.zshrc`（nvm 在這載入），`bash -c` 不讀任何 rc 所以 PATH 裡沒 node——跟 sandbox 無關

---

## 6. fd、fileglob 與 capability

**核心規則**：fd table 是 per-process，但 table 裡放的是 **指標**；fd 編號是那張 table 的 index，指向 kernel 全域、有 refcount 的 file object。「共享 fd」共享的是 object，不是編號。Seatbelt 的 `file-read-data` 檢查在 **open**（配鑰匙）那一刻，不在 **read**（轉門把）——fd 一旦存在就是一張 capability。

**關鍵細節**

```
cfprefsd 的 struct proc                        git 的 struct proc
  p_fd                                           p_fd
    [5] ──┐                                        [3] ──┐
          └────────▶  struct fileglob  ◀─────────────────┘      ← kernel 全域，refcount = 2
                        f_cred   = cfprefsd 的 cred（開檔當下蓋的章，掛在 object 上，不是掛在 git 身上）
                        f_offset = 讀寫位置（兩邊共用）
                        f_data   → vnode(~/.gitconfig)
```

- process **看不到任何位址**：A 只看到整數 5，B 只看到整數 3。table、fileglob、vnode 全住在 kernel 記憶體。搬運全由 kernel 做——kernel 只有一個地址空間，看得到所有 process 的 table；A 和 B 不能碰彼此的記憶體，但 kernel 可以碰所有人的
- refcount = `Arc<FileGlob>`：table 每一格是一個 `Arc` clone，`close()` 是 `drop`，fork 是整張 table 每格 `clone()`。A close 之後 B 的 fd 照樣能用，因為 A 只收回自己那一票。`unlink` 已開啟的檔案後還能讀，也是同一個理由：名字從目錄拿掉，vnode 的 count 沒歸零
- fd 傳遞後，git 的 ucred 從頭到尾沒變，它只是多了一張它自己造不出來的票。macOS 的 Powerbox（檔案對話框）、XPC 傳 file handle、`sandbox_extension` 全部建立在「sandbox 外的人開好 fd 再遞進去」這個機制上
- 對到 Rust：`OwnedFd` 是「抽屜裡那把鑰匙的所有權」，drop 時 close 讓 refcount−1

**三條讓兩個 process 指向同一個 object 的路**

| 路 | 誰做 | 結果 |
|---|---|---|
| `fork()` | kernel 複製整張 table | 父子所有 fd 指同一批 object，offset 共用 |
| `dup2(5, 3)` | 同一 process | 同一張 table 兩個 index 指同一 object |
| `sendmsg(SCM_RIGHTS)` | 透過 unix socket | kernel 在收方 table 找空格，塞進同一個 object 的指標 |

---

## 7. XPC 代辦與 `sandbox_check`：信任來自 kernel 蓋章

**核心規則**：XPC 是 Apple 在 Mach message 上包的 user-mode IPC 框架；某些資源不由你的 process 自己碰，而是由 launchd 管理的 daemon「擁有」，你送訊息請它代辦。Seatbelt 只檢查 **你這個 process 自己發的 syscall**，所以走 daemon 時撞的 operation 從 `file-read-data` 變成 `mach-lookup`，路徑規則完全失效，要改用服務名稱規則。

**關鍵細節**

```
直接讀（git 讀 ~/.gitconfig）：
  git ──open("/Users/admin/.gitconfig")──▶ kernel ──▶ 檔案

經 daemon（defaults 讀 plist）：
  defaults ──bootstrap_look_up("com.apple.cfprefsd.agent")──▶ kernel 查 launchd 名冊   ← mach-lookup 檢查點
  defaults ──mach_msg(請幫我讀 com.apple.dock)──▶ kernel ──▶ cfprefsd
  cfprefsd ──open(~/Library/Preferences/com.apple.dock.plist)──▶ kernel ──▶ 檔案        ← 用 cfprefsd 自己的 label
  cfprefsd ──mach_msg(回覆)──▶ kernel ──▶ defaults
```

- 這是後門：同 uid、不同 label，daemon 代讀繞過了 client 的路徑規則。補法 **不是把 profile 傳給 daemon**（profile 住在 kernel 拿不回來；更根本的是 client 自己傳的任何東西都不可信）
- 補法是 `sandbox_check`：kernel 轉送 Mach message 時在訊息上蓋 **audit token**（含 sender pid、uid），client 偽造不了；daemon 從 token 取 pid，問 kernel「如果是那個 pid 自己做，你會放行嗎」，kernel 拿那個 pid 的 label 跑一次判定、不真的 open。等於把 sandbox 判定「借」到 daemon 身上執行
- label 只往下傳、不橫傳是刻意的：如果 IPC 會把限制「傳染」給對方，系統 daemon 跟一個 sandboxed client 講一句話就被鎖死。代價是每個代辦型 daemon 得自己負責呼叫 `sandbox_check`，這是人為紀律不是 kernel 強制——Apple 自己的 daemon 漏做過好幾次，都變成 sandbox escape CVE
- osascript 正是這種情況：要找 launchservicesd 和目標 app 的 Apple Event port，整條路上沒有一個路徑可以加白名單，env 變數救不了（`GIT_CONFIG_GLOBAL` 那招只對「側讀路徑」有效）

---

## 8. DAC / MAC / VFS 三者關係

**核心規則**：VFS 是「場所」，DAC 和 MAC 是場所裡的兩道關卡——而且 MAC 這道關卡不只設在 VFS，它設在所有子系統。

**關鍵細節**

```
syscall 入口   open / read / connect / execve / mach_msg ...
              │
MAC framework  TrustedBSD MAC hooks          ← 橫向切過所有子系統的「檢查點插座」
              │  └ policy modules：Sandbox.kext、AMFI、Quarantine …
              │
VFS           namei → vnode → VNOP_*         ← 檔案子系統的抽象層（翻譯和分派，本身不做准駁）
   ├ DAC      vnode_authorize / kauth       ← 只存在於 VFS（和少數 IPC 物件）
   └ FS       APFS / HFS+ / devfs / NFS
network       socket / tcp / unix domain     ← 沒有 DAC，但有 MAC hook
Mach IPC      bootstrap / port rights        ← 沒有 DAC，但有 MAC hook
```

- **VFS**（Virtual File System）：把路徑字串變成 vnode（`namei()`），再用統一介面分派給底層 FS。DAC 和 MAC 的檢查是插在它的流程中間
- **DAC**（Discretionary）：**擁有者自己決定**。資料存在檔案的 inode（owner / mode / ACL），判定是 `cred.uid/gid` 對檔案的 owner / mode；`chmod` 就是擁有者行使「自主」；root 跳過
- **MAC**（Mandatory）：**政策決定，擁有者不能放鬆，root 不能退出**。資料存在 process 的 label（Seatbelt）或物件的 label（SELinux 用 xattr）；`chmod 777` 沒用，因為 MAC 不看 mode
- 一次 open 的兩道關卡：

```
open("/Users/admin/.gitconfig")
 └ VFS namei → vnode
   ├ mac_vnode_check_open   MAC：cred.cr_label → Sandbox.kext → 查 bytecode     ← sandboxed git 死在這，EPERM
   ├ vnode_authorize        DAC：cred.uid 對 vnode owner/mode                  ← Terminal 開 /etc/master.passwd 死在這，EACCES
   └ VNOP_OPEN → APFS
```

- MAC 在 DAC 前面：結果上兩個是 AND，順序影響的是（1）兩個都 deny 時看到哪個 errno，（2）policy 在任何可能有副作用的檢查之前先否決
- 為什麼要兩套：DAC 的根本假設是「擁有者可信」，在「同一個 uid 底下跑著不可信的程式」時整個崩掉——sandbox 內的 git 和 Terminal 內的 git **uid 一樣、檔案 owner 也一樣**，DAC 一條都分不出來。MAC 回答的是「同一個身分、不同信任等級」這個 DAC 沒有的維度；反過來 multi-user 的「我的檔案你不能看」還是 DAC 在管

**對照表**

| | DAC | MAC（Seatbelt） |
|---|---|---|
| 誰決定 | 檔案擁有者 | 政策制定者（這裡是 Claude Code 產的 profile） |
| 資料存哪 | 檔案的 inode（owner / mode / ACL） | process 的 `ucred.cr_label` |
| 判定依據 | 「你是誰」（uid / gid） | 「你被允許做什麼」（operation + filter） |
| root | 跳過 | 照查 |
| 覆蓋範圍 | 檔案、少數 IPC 物件 | 檔案、網路、Mach IPC、exec、signal、sysctl… |
| 擁有者能改嗎 | 能（chmod） | 不能 |
| 程序自己能改嗎 | 不能放寬（setuid 只能降） | 不能放寬（sandbox_init 只能緊） |
| errno | `EACCES` Permission denied | `EPERM` Operation not permitted |
| 實作 | `vnode_authorize` / kauth | MAC framework → Sandbox.kext |
| Linux 對應 | 同一套 + capabilities | LSM：SELinux / AppArmor / Landlock |

**診斷表（看錯誤字串判斷誰擋的）**

| 字串 | errno | 誰 | 層 |
|---|---|---|---|
| `Operation not permitted` / `unable to access '/Users/admin/…'` | EPERM | Seatbelt（MAC） | kernel |
| `Permission denied` | EACCES | rwx / ACL（DAC） | kernel |
| `No such file or directory` | ENOENT | 路徑不存在 | kernel |
| `-600` / `XPC connection invalid` / `Couldn't communicate with a helper application` | — | mach-lookup / appleevent 被擋 | kernel，但不是路徑 |
| `command not found` | — | shell 啟動檔差異（`bash -c` 不讀 rc） | user |
| `a terminal is required to read the password` | — | sudo 沒有 tty | user |
| timeout、exit code 1 | — | 不是 sandbox | — |

> **補強**：`Operation not permitted`（EPERM）在 macOS 上不只 Seatbelt 一個來源。至少三個 policy 都掛在同一個 MAC hook 上、都回 EPERM、都不看 rwx：
>
> | 來源 | 判定依據 | 誰決策 / 誰強制 | 典型路徑 | root 能過？ |
> |---|---|---|---|---|
> | Seatbelt | 發 syscall 的 process 自己的 `cr_label` | profile（harness 出題）/ kernel | 白名單外的任何路徑 | 否 |
> | TCC（隱私保護） | **responsible process** 有沒有被使用者授權——子 process 算到啟動它的 app 頭上，sandbox 內的 zsh 算到 Claude.app | tccd（user daemon）決策 / kernel 的 MAC hook 強制 | `~/Desktop` `~/Documents` `~/Downloads` `~/Library/Mail` … | 否 |
> | SIP（rootless） | vnode 的 `SF_RESTRICTED` flag + 路徑在保護清單 | Apple 寫死 / kernel | `/System` `/usr`（`/usr/local` 除外）`/bin` `/sbin` | 否（這正是它的目的） |
>
> 分辨法：同一條指令帶 `dangerouslyDisableSandbox` 重跑仍 EPERM → 不是 Seatbelt；路徑在 TCC 保護目錄 → 先懷疑 TCC（Terminal 裡沒授權過的 `ls ~/Documents` 一樣 EPERM）；`ls -lO` 看到 `restricted` → SIP。三者都是 Mandatory，所以 `chmod` 和 `sudo` 對它們全部無效——這是上面對照表「root 照查」那一格的三個實例，也是「MAC framework 上不只 Sandbox 一個 policy module」的具體樣子。

---

## 9. Claude sandbox 的兩層與多 agent：access control 不是 concurrency control

**核心規則**：Claude Code 的 sandbox 有兩層，兩層都只回答「能不能」。第一層是 kernel 強制（macOS 是 Seatbelt 的 MAC；Linux / WSL2 是 namespace + seccomp），第二層是 harness 自己的 permission / path 規則。kernel 眼裡沒有「agent」這個單位：每次 Bash 呼叫產生的那棵 process tree 共用一張 label，兩次呼叫、兩個 session、兩個 agent，都只是各自帶 label 的 process。**sandbox 管誰能碰什麼，不管誰先誰後**；要協調兩棵 process tree，得靠站在兩棵樹之上的東西。

**關鍵細節：兩層各管什麼**

```
                     誰決定                  管什麼                        不管什麼
─────────────────────────────────────────────────────────────────────────────────────────
第一層 kernel 強制    profile（harness 產）    Bash 子樹的 open/exec/connect  誰先誰後、同時寫同一檔
                     由 kernel 執行            （macOS Seatbelt；Linux/WSL2 namespace+seccomp）
第二層 harness        permission 規則、        要不要呼叫這個 tool；         MCP server 放行後做的任何事
                     path 規則                Read/Edit/Write 的路徑
不在任何一層          —                       MCP server process 本身       —
```

- 第一層的通用說法是「kernel 強制」，不是「MAC」：macOS 是 MAC policy 過濾 syscall，Linux 是 namespace（隔離手法）+ seccomp（syscall 過濾）——位置相同、手法不同
- MCP server 兩層都不管：harness 只決定「這個 tool call 要不要執行」，放行後 MCP process 想碰什麼都可以；app 層有路徑白名單的只有 Read / Edit / Write（harness 這一側的設計記在 rookie-llm Case 02，此處只記 kernel 看到什麼）
- 兩個 agent 都被允許寫同一檔，就是兩個 process 對同一 inode `open(O_WRONLY)`，之後的交錯跟沒 sandbox 一樣；bubblewrap bind mount 進去的也是同一個真實目錄。協調要靠 `flock`、git worktree（Claude Code Agent tool 的 `isolation: "worktree"`）、分目錄

**flock：鎖掛在 fileglob，不掛在 process、fd 或 label**

題：兩個 agent 的 sandbox 都允許寫 `/repo`。A `open` 後 `flock(LOCK_EX)` 鎖住 `foo.rs`；B 在 Seatbelt 下 `open` 同一檔並 `flock(LOCK_EX)`。(a) sandbox 擋 B 的哪一步？(b) B 的 flock 是失敗還是等待，由誰決定？(c) A 在 B 等待期間 fork 出 C，持有者是誰？A 退出後 B 會醒嗎？

- (a) 都不擋。Seatbelt 檢查點在 `open`，B 的 label 允許；`flock` 沒有 MAC hook
- (b) 預設 blocking：B 的 thread 離開 CPU 掛在鎖的 wait list（sleep，不在 run queue），不是失敗；加 `LOCK_NB` 才回 `EWOULDBLOCK`。由 caller 的 flag 決定
- (c) 持有者從頭到尾是 `fileglob_A`（open file description），不是 process、不是 fd、不是 label。fork 讓 C 多一條指向它的 fd，count 1→2；C 不排隊、fork 在 B 等待前後結果一樣。A 退出 count 2→1，鎖不釋放，B 繼續等；C 關掉最後一個 fd 或 `LOCK_UN`，kernel 在 C 的 thread 上解鎖並把 B 標 runnable，之後 scheduler 才跑 B（時間軸見 §14-E）

| 情境 | 結果 | 為什麼 |
|---|---|---|
| A fork 孩子用同一 fd | 孩子也持有；A 退出鎖還在 | 同一 fileglob |
| A `dup(4)` 得 5，對 5 flock | 不衝突 | 同一 fileglob |
| A 自己再 `open` 得 6，對 6 flock | A 跟自己打架卡死 | 新 fileglob，owner 不同 |
| C `flock(4, LOCK_UN)` | A 也失去鎖 | 解鎖單位是 fileglob |
| 多個等待者 | 全部標 runnable，誰先跑誰贏 | XNU `lf_wakelock` 不保證 FIFO |
| 第三者從不 flock 直接 write | 寫得進去 | advisory；sandbox 也救不了 |

- 對照 `fcntl(F_SETLK)` / POSIX lock：owner 是 process；該 process 對那檔案**任何**一個 fd close，鎖全消失。兩者都跟 label 無關：sandbox 在 ucred 上，鎖在 vnode 上，kernel 刻意不讓兩個軸互相知道

**git worktree 與 `.lock` 協定：站在所有 process 之上的仲裁者只有檔案系統**

- worktree 分開的只有工作目錄、HEAD、index（各在 `.git/worktrees/<name>/`）；`objects/`、`refs/`、`packed-refs`、`config` 共用。兩個 agent 同時 commit，都在往同一個 `objects/` 寫、都在改 `refs/heads/`
- 不打架是靠協定：objects 是 content-addressed（temp 寫完 rename，已存在就跳過，天然 idempotent）；ref 更新用 `open(".git/refs/heads/<br>.lock", O_CREAT|O_EXCL)` → 寫 sha → `rename` 原子取代。`O_EXCL` 是整套互斥的原子核心。不同 branch 不同 lock 檔名，零交集；同 branch 第二人 `EEXIST` → `Unable to create '...lock': File exists`，快速失敗不壞資料。共用碰撞點只剩 `packed-refs.lock`、`config.lock`、`gc.pid`
- sandbox 在這裡的貢獻是零，只會弄壞：不給寫 `.git/` → EPERM；給 create 不給 unlink → rollback 時 `.lock` 留在磁碟，之後每個 git 指令 `File exists`（Cowork 的 `device_bash` 預設不允許刪檔，就會出現這個）
- git 選檔案系統的 `O_EXCL + rename` 當鎖，是因為需要站在所有 process 之上（跨機器也成立）的仲裁者，而那只有檔案系統。協調兩棵 process tree 的東西，必須站在兩棵樹之上

---

## 10. Windows 與 WSL2 對照：identity-based 沒有「掛在 process 上的 policy」，WSL2 是第二顆 kernel

**核心規則**：Seatbelt 是 policy-based——bytecode 掛在 process 上，kernel 每個 syscall 跑它。Windows NT 是 identity-based——access token 裡的 SID / privilege / integrity level，對物件 security descriptor 的 DACL + mandatory label，由 Security Reference Monitor 在開 handle 時比對。NT 沒有「對這個 process 說某路徑以下不准」的語法，同樣效果要用 token + job + ACL 副作用 + WFP + 攔截層五樣湊。WSL2 不是「Windows 的 Linux 模式」，是 Hyper-V 輕量 VM 裡一顆 Microsoft build 的真 Linux kernel——所以 bubblewrap + seccomp 直接可用。

**關鍵細節：Windows**

現況（2026-11）：官方文件寫「The sandbox runs on macOS, Linux, and WSL2. On native Windows, Claude Code runs commands unsandboxed.」WSL2 走 bubblewrap；native Windows sandbox 是 open feature request。

| macOS | Windows | 差在哪 |
|---|---|---|
| `ucred` | access token | 都是 per-process、建 process 時決定、子 process 繼承 |
| `cr_label`（policy） | 無直接對應；最近的是 integrity level（MIC）、restricting SIDs、AppContainer SID | 都是「改身分」不是「掛程式」 |
| SBPL profile | 無；路徑規則寫在資料夾 ACL 或由 broker 在 user mode 判 | 最根本的差異 |
| `sandbox_init()` | `CreateRestrictedToken` → `CreateProcessAsUser` | process 不能換自己的 token，只能給孩子 |
| fork + exec 繼承 | `CreateProcess` 一步，token / job / handle 繼承都在 `STARTUPINFOEX` attribute list | 沒有 fork，沒有 `pre_exec` 空窗 |
| 血緣 | Job object | Windows 更強：明確 kernel 物件，kill-on-close、禁脫離 |
| fd / fileglob | HANDLE / kernel object | 同模型 |
| `SCM_RIGHTS` | `DuplicateHandle` | broker 直接塞進目標 table |
| 檢查在 open 不在 read | 一樣：`SeAccessCheck` 在 `NtCreateFile` | handle 就是 capability |
| XPC daemon + `sandbox_check` | broker + `AccessCheck()` | Chromium 的 broker/target 模型 |
| `EPERM` vs `EACCES` | 都是 `ERROR_ACCESS_DENIED (5)` | 診斷表失效 |
| App Sandbox（entitlements） | AppContainer（capability SIDs） | 可對任意 process 套 |
| Docker / VM 層級 | Windows Sandbox（Hyper-V）、WSL2 | 隔離不是過濾 |

Windows 上達到同樣效果的兩條路：

- **路 A：AppContainer（身分法）**——孩子帶 AppContainer SID，沒有任何 ACL 認得它 → 天然 default-deny；放行某資料夾 = 在那個資料夾的 ACL 加 ACE（寫在磁碟上的持久狀態，process 死了還在，harness 要清）；網路由 WFP 依 SID 擋，連 loopback 都要 `CheckNetIsolation LoopbackExempt`
- **路 B：broker / target（capability 法）**——孩子 token 弱到什麼都開不了，`CreateFile` 被攔截轉 IPC 給 broker（harness），broker 在 user mode 查路徑政策、自己開、`DuplicateHandle` 塞回去。就是「cfprefsd 開 fd 再遞進來」。要安全得補兩件事：target 不經 broker 真的開不了任何東西；broker 的 integrity 比 target 高

Anthropic 選 WSL2 的理由：macOS 是「一張文字、一個 CLI、kernel 負責一切」；Windows 是 token + job + ACL 副作用 + WFP + 攔截層五樣組起來，政策判定還有一部分回到 user mode。WSL2 是真 Linux kernel，bubblewrap + seccomp 直接可用。代價：跨 VM 檔案系統（9P）慢、路徑空間不同、Windows 原生工具不在裡面。shell 是 `bash.exe -c` / `pwsh -Command` 不影響任何一行——綁的是 process 不是 shell。

**關鍵細節：WSL2**

```
                WSL1（2016）                                 WSL2（2019 起，預設）
──────────────────────────────────────────────────────────────────────────────────────
kernel 數量      1：NT kernel                                2：NT + Linux
Linux 程式怎麼跑 pico process：lxcore.sys 在 NT 裡翻譯 syscall   真的 Linux process
相容性          翻不到的 syscall 失敗（namespace、seccomp 多半不行） 100%
檔案系統        直接 NTFS                                     自己的 ext4 vhdx；Windows 磁碟經 9P 掛到 /mnt/c
類比            Wine 反方向；跟 Seatbelt 一樣住在 syscall 邊界   VM，同 Docker Desktop 的 LinuxKit VM
```

WSL1 跟 Seatbelt 同形狀（syscall 入口插一層），目的是翻譯不是過濾；被放棄是因為翻譯層補不完——bubblewrap 要的 `unshare` / `seccomp` / `pivot_root` 都在補不完那堆裡。

```
Windows user      wsl.exe / wslservice        啟動 VM、接 console
Windows kernel    Hyper-V + vmbus             切 VM、分 CPU / 記憶體、host↔VM 通道
═══ VM 邊界 ═══
Linux kernel      linux-msft-wsl              namespace、cgroup、seccomp 全在
  └ init          WSL 自己的 init             掛 /mnt/c、設網路、起 distro
Linux user        Ubuntu rootfs               bash、git、Claude Code、bubblewrap
```

| 跨 VM 管線 | 怎麼接 | 代價 |
|---|---|---|
| `/mnt/c` | 9P 走 vmbus，Linux 端 `drvfs` | 每個 open / stat 一次跨 VM RPC，慢一個數量級 |
| 網路 | 虛擬網卡 NAT 到 host（新版有 mirrored） | VM 內 localhost ≠ Windows localhost |
| 跑 `.exe` | binfmt_misc 攔截，interop 請 Windows 那邊 spawn | 那是 Windows process，不在任何 Linux sandbox 裡 |

- bubblewrap 的白名單用 mount namespace 讓路徑**不存在**（`ENOENT`）；Seatbelt 用 label 讓 open **不准**（`EPERM`）。網路同理：沒網卡的 network namespace 只留到 proxy 的路，`curl --noproxy` 得到 `Could not resolve host`
- interop 逃逸點與 launchctl 同形：寄信請外面的人開程式；解法也同形：sandbox 內看不到 interop 的 socket
- `/mnt/c` 上的 open：Linux 端 namei → 9P client → 跨 VM → Windows 9P server 用 **Windows token** `CreateFile` → NT SRM 查 NTFS ACL。Linux 這邊的 uid / seccomp 管不到 Windows 側 ACL；bwrap 沒 bind `/mnt/c` 則在 Linux namei 就 ENOENT

來源（Claude Code 現況）：[Claude Code sandboxing docs](https://code.claude.com/docs/en/sandboxing)、[Issue #46740 Native sandbox support for Windows](https://github.com/anthropics/claude-code/issues/46740)

---

## 11. 繼承的真相與原語：fork 複製、exec 清理、spawn 合併、thread 共用

**核心規則**：「fork 會把 fd、label、uid 繼承過去」不是一條規則，是 kernel 在三個 syscall 裡對 `struct proc` 每個欄位各自決定「複製 / 共用 / 重設」；exec 不是繼承是清理；只有 parent → child，而且是 fork 那一瞬間的快照。再往下一層，kernel 真正的原語只有三個動作——**建 process 容器、換映像、建 thread**；fork / execve / posix_spawn / pthread_create 在 macOS 與 Linux 各自是 syscall 還是 libc wrapper 並不相同，POSIX 只規定 user 看到的函數與語意。

**關鍵細節：每個欄位各自決定**

全是 kernel 層；user 層只能事先插旗（`FD_CLOEXEC`）。

```
fork()           新建 proc（新 pid）+ 一條 thread
                 共用指標：ucred（uid+label）refcount+1（copy-on-write：任一方要改時 kernel 先配新的一張）
                 深拷貝一層：fd table 新表、每格指同一 fileglob（CLOEXEC 旗照樣複製，fork 不看它）
                 COW：地址空間（含 env / argv）
                 複製值：cwd、umask、rlimit、pgrp、signal handler
                 跟著走：flock（掛 fileglob）   不跟：fcntl 鎖（owner 是 process）

execve()         不建任何東西。不動：pid、ucred（setuid bit 例外換 uid，label 不動）、cwd、umask、rlimit、pgrp、fd table 本身
                 動一下：帶 FD_CLOEXEC 的格子 close
                 重設：地址空間、env / argv（由參數給）、signal handler → default、thread → 一條

posix_spawn()    = fork 的複製/共用 + execve 的重設；中間空窗改由 file_actions / attr 宣告，kernel 代勞
                 Apple：POSIX_SPAWN_CLOEXEC_DEFAULT = 全部視為 CLOEXEC，只留 file_actions 明說的（白名單式）

pthread_create() 不是新 process。新建一條 thread（stack、暫存器）；其他全部共用（地址空間、ucred、fd table 同一張）
```

| | fork | execve | posix_spawn | pthread_create |
|---|---|---|---|---|
| 新 process？ | 是 | 否 | 是 | 否 |
| ucred（uid + label） | 共用 | 不動（setuid 換 uid） | 共用 | 共用 |
| fd table | 新表、指同物件 | 同表，關 CLOEXEC 格 | 新表，照 file_actions | 同表 |

記法：**fork 複製、exec 清理、spawn 合併、thread 共用**。label 每一欄都「在」，是因為它住在 ucred 裡，四個 syscall 沒有一個會動 ucred——Sandbox.kext 不需要寫任何繼承邏輯。

- 「往下」只有 parent → child；不往上、不橫向（橫向要 SCM_RIGHTS）。是 fork 那一瞬間的快照不是活連結：父之後 open 的新 fd 孩子看不到；只有同一 fileglob 的 offset / flock 是活的共用
- `FD_CLOEXEC` 住在 **fileproc（table 的格子）** 不是 fileglob：`dup()` 出的新格子旗是乾淨的（`dup3` / `F_DUPFD_CLOEXEC` 才帶）。語意：「這個 fd 是我私人的」——fork 出的孩子還跑我的碼所以要有，exec 之後是別人的程式不該看到
- 誰在用：zsh 內部 fd 全設 CLOEXEC、要給孩子的 `dup2` 到 0/1/2；Rust std 所有 `File::open` / socket 預設 `O_CLOEXEC`，`Command::spawn` 的孩子只拿 0/1/2，要傳 fd 得 `Stdio::from(file)` 或 `pre_exec` 清旗；Windows 每個 HANDLE 有 `HANDLE_FLAG_INHERIT` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` 白名單，同一模型
- `git log > out.txt 2>&1` 這一行就是 fork / exec 拆兩步的理由：子 zsh 在空窗裡 `dup2` 把 out.txt 的 fd 搬到 1、2，exec 之後 git 不知道原本那個 fd 存在過；posix_spawn 把這三步變成 `file_actions`（時間軸見 §14-F）

**關鍵細節：原語與 wrapper**

| 呼叫 | macOS | Linux | 層 |
|---|---|---|---|
| `fork()` | syscall `fork` | syscall `clone`（不帶共用旗） | kernel |
| `execve()` | syscall | syscall | kernel |
| `posix_spawn()` | **真 syscall** | 不是；glibc 用 `clone(CLONE_VFORK)` + `execve` 拼 | macOS kernel / Linux libc |
| `pthread_create()` | 不是；libpthread → `bsdthread_create` | 不是；glibc → `clone(CLONE_VM\|CLONE_FILES\|CLONE_THREAD…)` | libc，底下 kernel |

- kernel 真正的原語是兩個動作：**建 process 容器**、**換映像**（加上建 thread）。Linux 連 fork 都不是底層，`fork()` = `clone(SIGCHLD)`；**process 和 thread 是同一個 syscall，差別只在旗子**（`CLONE_VM` 共用地址空間、`CLONE_FILES` 共用 fd table、`CLONE_THREAD` 共用 pid）。上面那張「複製 / 共用」表在 Linux 就是 clone 的 flag 清單
- pid 跟 proc 走、tid 跟 thread 走：fork / spawn 配新 pid + 一條 tid；execve 兩者不變；pthread_create 只配新 tid。Linux 每條 thread 有 tid，process 的 pid 其實是 `tgid`，主 thread tid == pid
- `pthread_create` 是 wrapper 在 user、thread 在 kernel（kernel 配 tid、kernel stack、排進 scheduler）。真正「user 自己延伸、kernel 看不見」的是 goroutine（G）、tokio task；GMP 的 M 才是 clone 出來的 kernel thread。「pthread 是 user 延伸」若推成「kernel 不知道 pthread」就錯
- Apple 把 spawn 做成 syscall 的理由：每個 process 是一個 Mach task，fork 要複製 task 的 port 空間與 page table，貴；多 thread process fork 後只有一條 thread 活著，ObjC runtime / Mach port / dispatch queue 狀態不一致（Apple：fork 後不 exec 是 undefined）。syscall 讓 kernel 直接「建空 task → 載映像」，不經過複製父親。Linux 用 `CLONE_VFORK`「先共用、再換掉」達到類似效果
- 為什麼 Linux 需要 clone 而 XNU 不需要：XNU 底下是 Mach，task / thread 本來就是一等公民；Linux 從零寫、沒有 thread 概念，所以發明「用旗子決定共用多少」的統一模型。差異是血統與 thread 模型，不是開放程度。POSIX 規定 user 看到的函數與語意，四個都在裡面；不規定底下是不是 syscall
- Apple 真正的生態系控制點在 Mach 層：Mach API（`task_for_pid`、port 操作）被 entitlement / SIP 鎖緊、fork 不 exec 標 undefined 逼大家走 `posix_spawn` 與 XPC、Sandbox.kext 閉源。「kernel 開源但你碰不到的那些門」

---

## 12. 「透過 kernel」＝透過軟體：硬體只給門和隔間

**核心規則**：kernel 是 OS 的核心，是軟體。硬體沒有 API，有 ISA + 特權模式 + 記憶體保護；晶片不知道 pid、fd、label 是什麼——那些是 kernel 放在 RAM 裡的 C struct。硬體只保證 user mode 碰不到它們、想碰只能走 trap，而 trap 進去跑的是 kernel 的碼。

**關鍵細節**

```
硬體（CPU / MMU）提供                              kernel（OS 的核心，軟體）提供
──────────────────────────────────────────────────────────────────────────────
特權等級 user / kernel mode（ARM64 EL0 / EL1）      process、thread、pid、tid
trap 指令：svc #0x80（ARM64）、syscall（x86）        fd table、fileglob、vnode、ucred、label、bytecode
exception vector：trap 跳去哪                       syscall table：第 N 號對應哪個 C 函數
MMU / page table：哪些頁 user mode 碰不到            fork / execve / clone 的實作
中斷、timer                                         scheduler、wait queue、flock 的 lock list
```

- 整條 sandbox 線講的東西，全在 trap 之後 kernel 程式碼那一步（時間軸見 §14-G 的 t3）；硬體只負責切模式與跳 vector
- 硬體只做「隔離 + 一扇門」是刻意最小化，讓 OS 自由設計 process 模型；反過來 kernel 的保證依賴那扇門是唯一入口（Meltdown 漏了一條路，上面所有 label 保證作廢）
- 比喻：kernel 是政府機關的公務員與制度，硬體是大樓——門禁與一扇申辦窗口。換 OS 進同一棟樓，門禁一樣、制度全換

---

## 13. Mach：XNU 底層的 kernel

**核心規則**：Mach 管 **task、thread、虛擬記憶體、port（IPC）** 四樣；process、fd、檔案、signal、socket 全是 BSD 層架在它上面做的。macOS 的 kernel 是兩個 kernel 疊在一起（hybrid：BSD 與 Mach 同一地址空間，不是真 microkernel）。port 與 fd 同構：right 是 kernel 發的 capability，偽造不了、只能被傳遞——整套 XPC / launchd / entitlement / Seatbelt 的 Mach 規則 / audit token 都靠這一點。

**關鍵細節**

```
user           程式、libc、libpthread、libxpc
═══ syscall 邊界（BSD syscall 與 Mach trap 走不同號碼）═══
BSD 層         proc、pid、ucred、fd table、VFS、socket、signal、pipe、flock、MAC framework   ← POSIX 住這；每個 proc 包一個 Mach task
Mach 層        task（地址空間 + port 空間）、thread、vm_map（page、COW）、port 與 message、scheduler
IOKit          驅動框架（C++）
硬體
```

| Mach 概念 | 是什麼 | 撞到它的地方 |
|---|---|---|
| task | 地址空間 + port 名字空間的容器，沒有執行能力 | fork 貴、posix_spawn 做成 syscall |
| thread | 執行單位，屬於某 task，Mach 原生 | `pthread_create` → `bsdthread_create`；Linux 要 clone 是因為沒這層 |
| vm_map | task 的頁表抽象；COW、mmap | fork 的 COW |
| port | kernel 管的單向訊息佇列；持有者拿的是 right（send / receive），是 capability | `mach-lookup`、XPC、osascript 被擋、launchd 名冊、audit token |

- port 與 fd 同構：port = 信箱；receive right 只有一個 task 持有（信箱主人）；send right 可多人持有、沒有就不知道信箱存在；每個 task 一張「整數 → right」表，整數是你的抽屜編號
- 拿 send right 的三條路：父 task spawn 時給（bootstrap port、task self port）、別人在訊息裡夾一個給你（同 SCM_RIGHTS）、問 launchd `bootstrap_look_up(name)`——這一步就是 Seatbelt `mach-lookup` 攔的地方（時間軸見 §14-H）
- Apple 留著 Mach 的原因：microkernel 願景沒完全實現，但 port 這個 capability 式 IPC 成了整套安全架構骨幹——XPC、launchd、entitlement、Seatbelt 的 Mach 規則、audit token 全靠「right 是 kernel 發的、偽造不了、只能被傳遞」。跟 fd capability 同一哲學，推廣到所有 IPC

---

## 14. 思維鏈

### A. Claude Code 的 Bash 跑 `git log`，到 EPERM

| t | thread / mode | 跑誰的程式碼 | 發生什麼 |
|---|---|---|---|
| t0 | Claude.app 主 thread / user | Claude Code（Node，label=NULL） | 讀 settings，組出 SBPL 字串；spawn `sandbox-exec -p "<SBPL>" zsh -lc 'git log'` |
| t1 | sandbox-exec / user | `/usr/bin/sandbox-exec` | `main()` 解析 `-p`，呼叫 `sandbox_init_with_parameters()` |
| t2 | sandbox-exec / user | libsandbox | TinyScheme 執行 SBPL → bytecode |
| t3 | sandbox-exec / kernel | libsandbox → `__mac_syscall` | 進 kernel |
| t4 | sandbox-exec / kernel | Sandbox.kext | 驗證 bytecode，掛到此 process 的 `ucred.cr_label`；回 user。**Claude Code 從此不在迴路** |
| t5 | sandbox-exec / user→kernel | `execvp("zsh")` | `process-exec` 檢查 → allow；映像換成 zsh，pid / ucred / label 不動 |
| t6 | zsh / user | `/bin/zsh -l` | 讀 `~/.zprofile`、`~/.zshrc`（nvm 在這載入） |
| t7 | zsh / user→kernel | `fork()` | kernel 複製 proc：新 pid，ucred 同一張（refcount+1，label 跟著），fd table 複製 |
| t8 | 子 zsh / user→kernel | `execve("/usr/bin/git")` | `process-exec` 檢查 → allow；換成 git 映像，label 保留 |
| t9 | git / user | git 啟動碼 | 不管你下什麼子指令，先讀 global config |
| t10 | git / user→kernel | `open("/Users/admin/.gitconfig", O_RDONLY)` | trap 進 kernel |
| t11 | git / kernel | VFS `namei` → `mac_vnode_check_open` | Sandbox.kext 拿 git 的 label，查 `file-read-data`：`/Users/admin` deny、`/Users/admin/work` 不符 → deny |
| t12 | git / kernel→user | — | `open()` 回 −1，`errno = EPERM`（DAC 根本沒跑到） |
| t13 | git / user | git config loader | `ENOENT` 會當沒這檔案繼續；`EPERM` 視為環境異常 → `fatal: unable to access '/Users/admin/.gitconfig': Operation not permitted`，exit |
| t14 | zsh / user | zsh | `waitpid` 返回；`-c` 沒有下一行，exit。**這個 zsh 死了，下次 Bash 呼叫從 t0 重來** |
| t15 | Claude.app / user | Claude Code | 收到 exit code 與 stderr，只看得到結果，不知道 t11 發生過 |

帶 `GIT_CONFIG_GLOBAL=/dev/null XDG_CONFIG_HOME=$TMPDIR` 時 t9 不會去 open 那個路徑，t10–t13 整段消失。

### B. SCM_RIGHTS 傳 fd：指標從 A 到 B，從來沒經過 user mode

```
      user (A 的視角)              │ kernel                                             │ user (B 的視角)
───────────────────────────────────┼────────────────────────────────────────────────────┼──────────────────
t0  A: open(~/.gitconfig) → fd 5   │ 建 fileglob，f_cred=A，count=1，放進 A->p_fd[5]       │
t1  A: sendmsg(sock, {fd:5})       │                                                    │
t2                                 │ A 的 thread：讀 A->p_fd[5] 得 fileglob*；count=2；   │
                                   │ 指標放進 socket 的 kernel buffer                    │
t3  A: sendmsg 返回 0              │                                                    │
t3' A: close(5)                    │ count=1（sockbuf 還持有）→ 物件活著                  │
                                   │ ── 指標躺在 kernel 記憶體，誰的 table 都沒有它 ──    │
t4                                 │                                                    │ B: recvmsg(sock)
t5                                 │ B 的 thread：從 sockbuf 取出 fileglob*               │
t6                                 │ 在 B->p_fd 找最小空格 [3]，寫入；sockbuf 放手         │
t7                                 │                                                    │ B: recvmsg 返回「fd 3」
t8                                 │ B 的 thread：read(3) → B->p_fd[3] → fileglob → vnode  │ B: read(3) ✅
                                   │ Seatbelt 的 file-read-data 檢查點在 open 不在 read   │
```

B 的 ucred 從頭到尾沒被碰過。t2 和 t5 是兩條不同 thread、各自在 kernel mode 跑 kernel 程式碼，交會點是 kernel 自己的一塊 buffer。

### C. sandbox 內 `sudo whoami`

```
t0  zsh (uid 501, label)        fork+exec sudo                  process-exec 檢查：允許
t1  kernel                      看到 S_ISUID + owner root → 新 ucred：euid=0, ruid=501, cr_label 不動
t2  sudo (euid 0, label)        沒有 tty 可以問密碼 → "sudo: a terminal is required…"，exit   ← 第一個死因不是 sandbox
    ── 若用 sudo -S 從 stdin 餵密碼 ──
t3  sudo (euid 0, label)        驗密碼 → PAM → mach-lookup "com.apple.opendirectoryd"
t4  kernel / Sandbox.kext       拿 sudo 的 label → (deny default) 沒開這個 name → deny     ← 擋的是 kernel，依據是 sudo 自己的 label
t5  sudo                        認證失敗，exit
    ── 若 profile 連 t3 都放行 ──
t6  sudo                        setuid(0)，execve whoami
t7  whoami (uid 0, label)       印 root ✅
t8  whoami                      open("/Users/admin/.ssh/id_rsa")：VFS 因 uid 0 跳過 rwx ✅ → MAC hook 看 label ❌ EPERM
```

### D. `launchctl` 逃逸與堵點

```
t0  zsh (label)          fork+exec launchctl                              → launchctl 繼承 label
t1  launchctl (label)    bootstrap_look_up(launchd 的管理 service)         ← mach-lookup 檢查點：Seatbelt 在這裡堵
t2  launchctl (label)    mach_msg 給 launchd：「start some.job」           ← 只是一封信
t3  launchd (pid 1, label=NULL)   posix_spawn("some.job")                  ← spawn 是 launchd 做的
t4  some.job             cred 複製自 launchd → label=NULL → 自由           ← 若 t1 沒堵，這就是完整的 escape
```

### E. 兩個 agent 的 flock：鎖掛在 fileglob，B 等到最後一個 reference 消失

```
t0  A              open(foo.rs) → fileglob_A（count=1）→ fd 4
t1  A              flock(4, LOCK_EX)            vnode lock list: [EX, owner=fileglob_A]
t2  B              open(foo.rs) → fileglob_B → fd 3
t3  B              flock(3, LOCK_EX)            衝突 → B 的 thread 掛 wait list，離開 CPU
t4  A              fork() → C                   C[4] → fileglob_A，count=2；C 此刻已「持有」
t5  A              exit                         count=1 → 不釋放 → B 續 sleep
t6  C              close(4) / exit              count=0 → 解鎖 → B 標 runnable
t7  scheduler      挑到 B                       B 重查 lock list → 寫入 owner=fileglob_B → 返回 0
```

Seatbelt 從頭到尾沒出場：t0 / t2 的 `open` 兩邊 label 都允許，`flock` 沒有 MAC hook。

### F. `git log > out.txt 2>&1`：fork / exec 之間的空窗在做什麼

```
t0  zsh        open("out.txt") → fd 7
t1  zsh        fork()                      孩子有 0,1,2,7、同一張 ucred(label)
t2  子 zsh     dup2(7, 1)                  [1] 指 out.txt 的 fileglob，旗乾淨
t3  子 zsh     dup2(1, 2)
t4  子 zsh     close(7)（或靠 CLOEXEC）
t5  子 zsh     execve("git")               kernel 關掉所有 CLOEXEC 格；0,1,2、label 留下
t6  git        write(1) → out.txt          git 不知道 7 存在過
```

t2–t4 就是 fork / exec 拆兩步的理由；posix_spawn 把它變成 `file_actions`。

### G. 一次 `fork()`：硬體只負責 t1 和 t4

```
t0  git / user (EL0)     libc fork() wrapper：syscall 號碼放 x16，執行 svc #0x80
t1  CPU（硬體）           看到 svc → 切 EL1、存 pc 進 ELR、跳 exception vector      ← 硬體到此結束
t2  kernel (EL1)         handler 讀 x16 → 查 syscall table → fork1()
t3  kernel (EL1)         配 struct proc、pid、ucred refcount+1、複製 fd table …（全是軟體、全在 RAM）
t4  kernel (EL1)         eret → 硬體切回 EL0
t5  git / user (EL0)     從 fork() 返回
```

整條 sandbox 線講的東西全在 t3。

### H. `bootstrap_look_up`：一個 send right 怎麼進到 client 的表

```
t0  client     bootstrap_look_up(bootstrap_port, "com.apple.cfprefsd.agent")   ← 本身是一封 mach_msg 給 launchd
t1  kernel     MAC hook：mach-lookup (global-name …) → Seatbelt 查 label → allow / deny（deny = -600 / XPC invalid）
t2  launchd    查名冊，把 cfprefsd 信箱的 send right 夾在回信
t3  kernel     把 right 插進 client task 的名字空間，回一個整數
t4  client     mach_msg(send, port=那個整數, body=「讀 com.apple.dock」)
t5  kernel     訊息放進 cfprefsd 信箱；附 audit token trailer
t6  cfprefsd   mach_msg(receive) 醒來；讀 token 取 pid；sandbox_check；代辦；回信
```

跟 §14-B 的 SCM_RIGHTS 同構：right / fileglob 指標只在 kernel 裡搬，client 從頭到尾只拿到一個整數。

---

## 15. 陷阱清單

標【模型】是概念錯、會導致錯誤預測；標【用詞】是說法不精確。

1. 【模型】❌ sandbox 只管檔案存取，程式本身能跑就好 → ✅ Seatbelt 管 syscall 類別：file、network、Mach IPC、process-exec、signal、sysctl…；exec 本身也是被查的 operation，能跑是因為 profile 允許 `process-exec*` → 為什麼：git 能啟動但讀 `~/.gitconfig` 被擋、osascript 能啟動但 `mach-lookup` 被擋，都是「exec 放行、後面的行為另算」。

2. 【模型】❌ 「合法」由 kernel 判定 → ✅ kernel 沒有合法的概念，只拿 profile 逐條比對；「合法」是 Claude Code 產的 default-deny 白名單定義的 → 為什麼：出題（user mode）和閱卷（kernel）分開；調 `/sandbox` 是在改考卷。

3. 【模型】❌ Claude Code 參與每次檢查 / 「claude 這隻 proc 會 block」 → ✅ `sandbox_init` 之後 Claude Code 完全不在迴路，沒有 callback；擋的是 kernel，依據是發 syscall 的那個 process 自己的 label → 為什麼：user mode 的 wrapper 可以被子 process 繞過，kernel 是所有 process 都躲不掉的交會點。

4. 【模型】❌ 每個 syscall 都撞一次 kernel check → ✅ 檢查掛在「取得資源」的 syscall（open、execve、connect、bind、mach_lookup），拿到 fd 之後的 read / write 不再查 → 為什麼：fd 是 capability；這是 Powerbox、XPC 傳 file handle、`sandbox_extension` 全部建立的基礎。

5. 【用詞】❌ git 去存取 `~/.git` 目錄 → ✅ `~/.gitconfig` 和 `~/.config/git/ignore`，且不管子指令是什麼都會讀 → 為什麼：git 對 config 檔容忍 ENOENT、不容忍 EPERM，所以 `GIT_CONFIG_GLOBAL=/dev/null` 能繞。

6. 【模型】❌ SBPL compile 出來跑在 sandbox-exec 上 → ✅ 跑 bytecode 的引擎是 kernel 的 Sandbox.kext；sandbox-exec 是 loader（編譯、`sandbox_init`、`execvp`），之後被 exec 取代而消失 → 為什麼：和 seccomp-bpf 同形狀：規則編成 bytecode 塞進 kernel，kernel 每個 hook 跑它。

7. 【模型】❌ Seatbelt 完全沒公開 API，Claude Code 是瞎子摸象，Apple 一改就斷 → ✅ 三層：entitlements 公開；`sandbox-exec` / `sandbox.h` 出貨但 deprecated（14 年沒拿掉，整個 macOS 工具鏈靠它）；SBPL 完整語法才是逆向；且 `(deny default)` 使 drift fail-closed → 為什麼：接線的路是 Apple 的，寫規則的知識是逆向的；不認識的東西一律 deny，壞的是功能不是安全。

8. 【模型】❌ XPC service 是另一個 kernel → ✅ kernel 只有一個；XPC 是 user-mode daemon 代辦；sandbox 看到的 syscall 從 `open` 變 `mach-lookup` → 為什麼：Seatbelt 只檢查你自己發的 syscall，daemon 的 open 在 daemon 自己的 label 下。

9. 【模型】❌ 把 SBPL profile 當上下文傳給 daemon 就能補後門 → ✅ daemon 從 kernel 蓋章的 audit token 取 pid，呼叫 `sandbox_check(pid, …)` 問 kernel → 為什麼：profile 住在 kernel 拿不回來；client 自己傳的任何東西都不可信；信任來自 kernel 蓋章的身分。

10. 【模型】❌ cfprefsd 是一個 user → ✅ 是一支 process（`/usr/sbin/cfprefsd`），由 launchd spawn；uid 和 label 是兩個獨立的軸 → 為什麼：git 和 cfprefsd 同 uid、不同 label，DAC 分不出它們，這正是 MAC 存在的理由。

11. 【模型】❌ 有兩個 launchd（root 的和 agent user 的） → ✅ 10.10 起只有 pid 1 一個，內部管 system domain 和每個使用者的 gui domain；daemon / agent 是 launchd 術語不是 user → 為什麼：GUI app、agent、daemon 的 parent 全是 pid 1。

12. 【模型】❌ 「不同 process 不能共享資源」所以 cfprefsd 在 sandbox 外 → ✅ process 之間能共享很多（檔案、pipe、shm、mach port）；不會跨 process 傳的只有 cred label；判準不是「不同 process」而是「祖先鏈有沒有 sandbox-exec」 → 為什麼：git 和 zsh 也是不同 process，git 照樣在內。

13. 【模型】❌ 繼承要靠寫檔之類的機制 / 看時間順序 → ✅ 只看 parent 是誰；zsh 拿到 label 之後 launchd 才重啟的 cfprefsd 一樣乾淨 → 為什麼：label 是 ucred 的欄位，fork 時複製，沒有其他傳播規則。

14. 【模型】❌ zsh 是「把某個腳本執行」 → ✅ zsh 是程式，`~/.zshrc` 才是腳本；每次 Bash 呼叫是全新的 `zsh -lc`，跑完就死；`bash -c` 不讀 rc 所以沒 nvm → 為什麼：`command not found` 是 shell 啟動檔差異，不是 sandbox。

15. 【用詞】❌ `exec zsh` 之後還被擋是因為「它是 sandbox-exec fork 出來的」 → ✅ exec 沒有 fork：同 pid、同 proc、同 ucred，只換映像 → 為什麼：sandbox-exec 自己也是 exec 成 zsh 的，整條鏈上第一次 fork 是 zsh 跑指令時。

16. 【模型】❌ 透過 sandbox 內的 launchctl 啟動的 job 會繼承 label → ✅ 真正 posix_spawn 的是 launchd（label=NULL），job 乾淨，這是完整 escape；Seatbelt 在 `mach-lookup` 堵 launchd 的管理 service → 為什麼：label 跟著 syscall 的執行者，不跟著意圖的發起者（又一次「發起者 = 執行者」的幻想）。

17. 【模型】❌ 用 sudo 就是想用 root 執行，所以 label 應該跟著清掉 → ✅ setuid exec 只改 `cr_uid`，`cr_label` 不動；root 只對 DAC 有意義 → 為什麼：MAC 的 M 是 Mandatory，連 root 都不能退出；否則任何 sandbox 都能靠 exec 一支 setuid 程式逃出去。

18. 【模型】❌ fd 傳過來後 git 會變成 cfprefsd 的狀態，因為 cred 跟著 exec / fork / spawn 帶入 → ✅ 這條路上沒有任何 fork 或 exec；git 的 ucred 沒變；`f_cred` 掛在 file object 上不是掛在 git 身上 → 為什麼：授權單位是「你手上持有什麼」，不是「你是誰」。

19. 【模型】❌ kernel fd table 是 global，每個 process 看到相同的記憶體位址 → ✅ process 只看到整數；table 是 per-process、object 是全域，兩者都在 kernel 記憶體；搬運由 kernel 在 A 的 thread 和 B 的 thread 上分兩次做 → 為什麼：user / kernel 層混放（反覆出現的習慣）——user mode 從來沒持有過那個指標。

20. 【模型】❌ A close 之後 B 拿到的 fd 不能用 → ✅ refcount：A 只收回自己那一票，socket buffer / B 的 table 還持有 → 為什麼：fileglob 就是 `Arc`，close 是 drop，物件活到 count 歸零。

21. 【用詞+模型】❌ VFS 就是讀檔；Terminal 的 zsh「label 沒有 cred」；沒 sandbox 也會因為 deny path 開不了 → ✅ VFS 是抽象層（namei + 分派），DAC 和 MAC 都插在它裡面；每個 process 一定有 cred，是 label 格為 NULL；沒 label 就沒有任何路徑規則，`/etc/master.passwd` 死在 DAC，errno 是 EACCES → 為什麼：MAC 先、DAC 後；兩者都 deny 時你看到的是 EPERM，只有 DAC deny 時才是 EACCES。

22. 【模型】❌ `command not found`、timeout、exit code 1 也當成 sandbox → ✅ 只有 `Operation not permitted` / `unable to access '/Users/admin/…'` / mach-lookup 類錯誤是 Seatbelt；其他是 shell 環境、tty、timeout → 為什麼：見 §8 診斷表，每個字串對應一個不同的檢查點或根本沒有檢查點。

> **補強**（對應第 22 條）：「只有 EPERM 類字串是 Seatbelt」要再收一層——EPERM 是 Seatbelt 的必要條件不是充分條件，TCC 與 SIP 走同一個 MAC hook、同樣回 EPERM（§8 補強）；分辨靠「關掉 sandbox 重跑是否仍 EPERM」。

以下 23–37 是續篇（§9–§13）那段撞到的。

23. 【模型】❌ app 層讓 MCP 只能操作白名單 → ✅ MCP server 兩層都不管，harness 只決定要不要呼叫這個 tool → 為什麼：MCP 是獨立 process，不經 sandbox-exec，harness 也沒對它的 syscall 做任何事；app 層有路徑白名單的只有 Read / Edit / Write。

24. 【用詞】❌ 第一層通稱 MAC → ✅ 通稱「kernel 強制」 → 為什麼：macOS 是 MAC policy 過濾 syscall，Linux 是 namespace（隔離）+ seccomp（syscall 過濾）——位置相同、手法不同。

25. 【模型】❌ 多 agent 都開 sandbox 就不會互相競爭 → ✅ sandbox 是 access control 不是 concurrency control → 為什麼：兩個都被允許寫同一檔，就是兩個 process 對同一 inode `open(O_WRONLY)`，之後的交錯跟沒 sandbox 一樣；協調要靠站在兩棵樹之上的東西（flock、worktree、分目錄）。

26. 【模型】❌ Windows 把 Seatbelt 換成某個 kernel API 就好 → ✅ Windows 是 identity-based，沒有「掛在 process 上的 policy」原語 → 為什麼：要組 token + job + ACL 副作用 + WFP，或 broker / target；政策判定有一部分回到 user mode。Anthropic 因此選 WSL2。

27. 【模型】❌ WSL2 是 Windows 的 Linux 相容層 → ✅ 那是 WSL1（pico process 在 NT 裡翻譯 syscall）；WSL2 是 Hyper-V VM 跑第二顆真 Linux kernel → 為什麼：翻譯層補不完 `unshare` / `seccomp` / `pivot_root`，而 bubblewrap 全靠它們。

28. 【模型】❌ B 的 flock 會「失敗」，因為 fd 在 B 自己的 table → ✅ 預設 blocking 是等待不是失敗（thread 掛 wait list）；鎖掛 fileglob 不是 fd / process / label → 為什麼：`flock` 沒有 MAC hook，Seatbelt 檢查點在 open；加 `LOCK_NB` 才回 `EWOULDBLOCK`，由 caller 決定。

29. 【模型】❌ A fork 出的 C 要「排隊」（A→B→C 或 A→C→B） → ✅ C 不排隊，fork 完成即共同持有；B 等到 `fileglob_A` 最後一個 reference 消失 → 為什麼：fork 讓 C 多一條指向同一 fileglob 的 fd（count 1→2），A 退出只是 count 2→1，鎖不釋放。

30. 【模型】❌ 不同 branch 的 worktree commit 結構上互不影響 → ✅ `.git/objects`、`refs` 共用；不影響是 `O_EXCL + rename` 協定做到的 → 為什麼：objects content-addressed 天然 idempotent；ref 更新靠 `.lock` 檔的 `O_EXCL` 互斥，不同 branch 不同檔名零交集。

31. 【模型】❌ 「fork 會把 fd、label、uid 繼承過去」是一條規則 → ✅ kernel 對每個欄位各自「複製 / 共用 / 重設」；exec 不是繼承是清理；只有 parent→child、是快照 → 為什麼：ucred 共用指標 refcount+1、fd table 深拷貝一層、地址空間 COW、cwd / umask 複製值；父之後 open 的新 fd 孩子看不到。

32. 【模型】❌ 底層就是 fork 和 execve → ✅ 底層是「建 process 容器 / 換映像 / 建 thread」三個動作；Linux 的底層是 clone → 為什麼：Linux 的 `fork()` = `clone(SIGCHLD)`，process 和 thread 是同一個 syscall、差別只在旗子。

33. 【模型】❌ pthread 是 user 延伸、kernel 不知道 → ✅ wrapper 在 user（libpthread → `bsdthread_create` / `clone`），thread 由 kernel 建與排程 → 為什麼：kernel 配 tid、kernel stack、排進 scheduler；真正 kernel 看不見的是 goroutine / tokio task。

34. 【模型】❌ Linux 的 posix_spawn 是 execve 派生的 syscall → ✅ glibc 用 `clone(CLONE_VFORK)` + `execve` 拼的；macOS 才是真 syscall → 為什麼：Apple 每個 process 是 Mach task，fork 要複製 port 空間與 page table 太貴，所以讓 kernel 直接「建空 task → 載映像」。

35. 【模型】❌ Apple 不開源、只要符合 POSIX → ✅ XNU 開源，閉的是 kext（含 Sandbox.kext）/ framework；fork / execve 是 1970 年代 Unix 原語，clone 才是 Linux 後來發明的 → 為什麼：差異來自 Mach 的 thread 模型（task / thread 本來就是一等公民），不是開放程度。

36. 【模型】❌ XNU 是更早的架構、Linux 根據它 tune 生態系 → ✅ Linux（1991）比 macOS（2001）早、從零寫、零共用碼；「類 Unix」= 共用 POSIX 介面 → 為什麼：macOS 是真 BSD 碼 + 真 Mach 碼，Linux 只參考介面；家譜見 `blueprints/os.md`。

37. 【模型】❌ 「透過 kernel」可能是硬體 API → ✅ kernel 是軟體；硬體只提供特權模式、trap、MMU；pid / fd / label 都是 RAM 裡的 struct → 為什麼：硬體沒有 API 只有 ISA；`svc` 之後跑的是 kernel 的 C 碼，整條 sandbox 線都在那裡。
