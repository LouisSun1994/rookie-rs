# Experience 01 — libc、ABI 與 Alpine：Debian 上編的 binary 為什麼在 Alpine「not found」

> 經驗不是筆記：起點不是一個想挖的主題，而是一個實際撞到的問題，往下挖到能解釋它為止。這一篇從「Rust binary 丟進 Alpine 容器跑不起來、shell 說 `not found`」出發，挖到 libc 住哪一層、API / ABI / ISA 三份契約、target triple 的五層檢查、Linux 唯一穩定的 syscall ABI、動態載入器與 `dlopen`，最後是「依賴」其實有四種、靜態連結只消掉一種。
> 記錄日期：2026-11-05｜當時定位：🥚 小白（仍未動手寫程式；這條線碰到 🐊 的 FFI / C ABI 與 ELF 載入）
> 起因：在 Debian 上用 Rust 預設 target 編的 binary，放進 Alpine 容器執行，`sh: /app/app: not found`。檔案明明在。
> 接在哪裡：note 04「`static` / `const` / `'static` / section / ELF」段的下半段（linker 之後、loader 這一側）；note 05 §5 與 §11 的「exec 換映像」。OS 藍圖：子系統 ②、模式 4（介面 vs 實作）與模式 6（名字空間）。
> §11 是這條線留下的十題腦筋急轉彎，答案是入庫時整理者推的（標 **補強**，未經導師確認）；對應的 checklist 條目在 [`blueprints/os-checklist.md`](../blueprints/os-checklist.md)。

---

## 0. 一句話版本

**能跑 = ISA 對 + 執行檔格式對 + kernel ABI 對 + 載入器找得到 + libc ABI 對（版本 ≥）**。Alpine 那次死在第四層（kernel `execve` 找不到 ELF 裡寫死的 glibc 載入器），根因在第五層（musl ≠ glibc）；程式連第一條指令都沒執行，shell 印的「not found」指的是載入器不是 binary。API 是給編譯器看的契約，ABI 是給 bytes 看的契約；前者違約會被擋，後者違約靠運氣。靜態連結消掉「我的 bytes 和別人的 bytes 要對上」，消不掉「我的 bytes 要讀誰的檔案、走誰的 syscall」。

## 1. 事件：發生了什麼

binary 在 Debian（Rust 預設 target `x86_64-unknown-linux-gnu`）編譯 → 動態連結 glibc，ELF 裡寫死兩件事：

```
$ readelf -l app | grep interpreter   → [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]   ← PT_INTERP
$ readelf -d app | grep NEEDED        → (NEEDED) Shared library: [libc.so.6]                            ← 要的 libc
```

Alpine 的載入器是 `/lib/ld-musl-x86_64.so.1`、libc 是 `libc.musl-x86_64.so.1`，兩個名字都對不上。

```
t0  sh (Alpine)        fork + execve("/app/app")
t1  kernel execve      讀 ELF header → PT_INTERP = /lib64/ld-linux-x86-64.so.2
t2  kernel             open("/lib64/ld-linux-x86-64.so.2") → ENOENT
t3  kernel             execve 回 -1, errno=ENOENT   ← 程式連第一條指令都沒執行
t4  sh                 印 "sh: /app/app: not found"  ← 誤導：不在的是載入器，不是 binary
```

- 它**沒有**在 Alpine 啟動；死在 kernel 的 `execve` 裡、還沒輪到載入器去找 libc。
- 當時的理解是「用到 libc = 用到 Unix kernel 裡的 libc，Alpine 的 libc 版本不同是因為底層 driver 不同」。這個模型預測不了為什麼 Debian 和 Fedora 之間能跑、Debian 和 Alpine 之間不能——同一顆 Linux kernel、同一顆 CPU。往下挖的順序：libc 住哪一層（§2）→ API 與 ABI 是什麼（§3）→ ISA 是哪一份契約（§4）→ 能不能跑到底在檢查什麼（§5）→ 誰承諾了什麼穩定（§6）→ 回頭解剖這次事件（§7）→ 解法（§8）→ 靜態連結之後還剩什麼依賴（§9）。

## 2. libc 住哪一層

```
user     你的 app
user     libc  ──┐  glibc（Debian / Ubuntu / RHEL）、musl（Alpine）、libSystem（macOS）、Bionic（Android）
                 │  工作：把 open() 這種 C 函數翻成 syscall 指令；外加 malloc、printf、pthread、DNS 解析…
═══ syscall 邊界 ═══   ← kernel 只看到 syscall 號碼與暫存器，不知道上面是哪家 libc
kernel   Linux / XNU / NT
```

- libc 不在 kernel 裡，kernel 裡沒有 libc。藍圖六大子系統圖裡 libc 在 trap 線**上方**那一行（user space）。
- 「libc」是職位名稱不是人名：POSIX 是職務說明書，glibc / musl 是兩個應徵上的人。都符合說明書，彼此不可互換。
- libc 從不碰 driver；driver 在 kernel 下面兩層。libc 只透過 syscall 跟 kernel 講話。

## 3. API vs ABI：同一份契約的兩個時期

```
編譯前（有原始碼）   API：函數叫 open、三個參數、回 int、語意是打開檔案
                     → 編譯器讀 header 就能對上，不需要任何人承諾位元怎麼擺
        ↓ 編譯
執行期（只剩 bytes） ABI：第一個參數放 rdi、回傳值在 eax、struct stat 第 48 byte 是 st_size、
                     符號叫 open@GLIBC_2.2.5、errno 放 TLS 偏移……
                     → 兩邊都沒有原始碼了，只能靠事先約好的擺法
```

**核心規則：每一個「分開編譯、執行期才接上」的邊界，都需要一份 ABI。** ABI 不是動作也不是元件，是規格（像插座規格、像協定文件）；串接是動作，ABI 是讓串接不用溝通就成立的約定。kernel 執行 binary 時不是「在串 ABI」，是雙方各自假設對方遵守。

| 邊界 | 兩造 | 契約內容 | 違約時怎麼死 |
|---|---|---|---|
| 你的 binary ↔ `libc.so` | 軟體 ↔ 軟體 | 符號名與版本、struct 佈局、型別大小、呼叫慣例、載入器路徑 | `version GLIBC_2.34 not found`、靜默讀到垃圾 |
| libc（或你直接）↔ kernel | 軟體 ↔ 軟體 | syscall 號碼、參數暫存器、kernel 期待的 struct 佈局、errno 編碼 | `ENOSYS`、行為亂掉 |
| 任何機械碼 ↔ CPU | 軟體 ↔ **硬體** | 哪些 byte 序列是指令、有哪些暫存器、記憶體模型 ＝ **ISA** | `SIGILL` |
| Rust crate ↔ Rust crate | 軟體 ↔ 軟體 | **沒有穩定 ABI**，cargo 全部從原始碼一起編 | 不會發生，因為沒有這個邊界 |
| Rust ↔ C（`extern "C"`、`#[repr(C)]`） | 軟體 ↔ 軟體 | 借用 C 的 ABI | FFI 的 UB 多半是 ABI 沒對齊 |

- ABI 跟硬體無關，跟「兩段分開編譯的東西在執行期碰面」有關。kernel ABI 的兩造都是軟體；只有 ISA 那份的對手是硬體。
- **ABI 違約多半是靜默的**：API 違約編譯器會擋；ABI 違約只有少數有標籤的地方會擋（ELF magic、`e_machine`、PT_INTERP 路徑、符號版本），其餘（struct offset、參數暫存器、型別大小）完全不檢查，讀到垃圾、亂跑、偶爾才崩。插座：形狀不對插不進（有檢查）；形狀對電壓不對，燒給你看（沒檢查）。
- Rust 對照：trait 是 API，記憶體佈局 + 呼叫慣例是 ABI。Rust 刻意無穩定 ABI → crate 一律從原始碼重編、不發 `.so`；C 靠穩定 ABI 發 `.so` 共用。`#[repr(C)]` / `extern "C"` 是「跨出 Rust 時改用 C 的 ABI」。
- 一句話：**API 是給編譯器看的契約，ABI 是給 bytes 看的契約；前者違約會被擋，後者違約靠運氣。**

## 4. ISA：CPU 對軟體的 ABI

```
指令編碼     哪些 byte 序列是合法指令、各自做什麼
             x86-64：變長 1–15 bytes，0x48 0x89 0xe5 = mov rbp, rsp
             AArch64：固定 4 bytes，0xd10043ff = sub sp, sp, #16
暫存器       x86-64：rax … r15（16 個通用）；AArch64：x0 … x30 + sp（31 個）
記憶體模型   x86 是 TSO（寫入幾乎不重排）；ARM 是弱序（note 04 的 atomic ordering 在 ARM 上真的有差）
特權機制     x86 的 ring 0–3 / syscall 指令；ARM 的 EL0–EL3 / svc 指令 ← 那扇門的長相由 ISA 定
```

CPU 不讀原始碼、不讀 ELF、不知道函數——從 pc 指的位址一個 byte 一個 byte 解碼。同一串 bytes 在 x86 是 `mov`、在 ARM 可能是 `sub` 或不合法。上層所有 ABI（參數放 rdi 還是 x0）都建在 ISA 之上。

**為什麼 x86 的 ELF 到 ARM 上是 `Exec format error` 不是「ELF 格式不對」**：ELF 是跨架構的容器格式，ARM kernel 看得懂 x86 的 ELF——看得懂到足以知道它不是給自己的。

```
t0  sh (ARM Linux)     execve("./app-x86")
t1  kernel binfmt_elf  讀前 64 bytes：magic 0x7f 'E' 'L' 'F' ✅、64-bit ✅、little-endian ✅   ← 格式看得懂
t2  kernel             讀 e_machine：62（EM_X86_64），我是 183（EM_AARCH64） ✗
t3  kernel             execve 回 ENOEXEC → sh 印 "Exec format error"
```

- kernel 在 t2 就擋，是為了死得乾淨：不查直接跳過去，CPU 會把 x86 bytes 當 ARM 指令解碼 → `SIGILL` 或亂跑。
- 這層可以被「代打」：Linux `binfmt_misc` 登記「e_machine = ARM 的交給 `qemu-aarch64`」→ Docker 在 x86 跑 arm64 image；Rosetta 2 是 macOS 在同一層做的事。macOS 的 Mach-O header 有 `cputype`，Universal binary = 一個檔案裝多個 ISA 的 slice，kernel 挑自己那片。

## 5. 能不能跑：五層檢查，單位是 target triple 不是 OS

```
x86_64 - unknown - linux - gnu
  │                  │       └─ libc ABI：glibc（gnu）/ musl / 無
  │                  └──────── kernel ABI + 執行檔格式：Linux + ELF / darwin + Mach-O / windows + PE
  └───────────────────────── CPU ISA：x86_64 / aarch64
```

編譯的機器和產物要跑的環境是兩回事：**target 決定產物，不是 host**。在 macOS 上 `cargo build --target x86_64-unknown-linux-musl`，產物在 Alpine 跑。

```
檢查順序            對不上時死在哪                       錯誤長相
──────────────────────────────────────────────────────────────────────────────
① CPU ISA           kernel execve 讀 ELF header 的 e_machine     "Exec format error"（ENOEXEC）
② 執行檔格式        kernel 認不認得 ELF / Mach-O / PE            同上；macOS 拿到 ELF 直接 ENOEXEC
③ kernel ABI        syscall 號碼與暫存器                         能啟動，第一個 syscall 回 ENOSYS 或亂掉
④ 載入器路徑        execve 找 PT_INTERP                           "not found"（ENOENT）← Alpine
⑤ libc ABI          載入器找 NEEDED、比對符號版本                 "version GLIBC_2.34 not found" / 靜默壞掉
（kernel 內實際順序：先認 magic（②）再查 e_machine（①），同一個函數）
```

在 x86_64 Ubuntu 編的 glibc 動態 binary，能跑的範圍 = **x86_64 + Linux + glibc 版本 ≥ 編譯時那版**：

| 目標 | 能跑？ | 死在哪 |
|---|---|---|
| Debian / Fedora / Arch（x86_64、glibc 夠新） | ✅ | — |
| 舊版 Ubuntu（glibc 比編譯時舊） | ❌ | ⑤ `version GLIBC_2.xx not found` |
| Alpine | ❌ | ④ 載入器路徑 |
| FreeBSD（也是 Unix） | ❌ | ③ syscall 號碼不同（有 Linux 相容層例外） |
| ARM Linux | ❌ | ① ISA |
| macOS | ❌ | ② 格式 |
| Windows | ❌ | 比 ② 更早：PE 載入在 user mode 的 ntdll，`not a valid Win32 application` |

- 「Unix」在這裡沒有用——FreeBSD 血統比 Linux 正統，照樣不能跑。
- 靜態 musl binary 拿掉 ④⑤，只剩 ①②③ → 任何 Linux 發行版都能跑，到 macOS 仍死在 ②。
- 跨 OS 必死，除非有人補一顆 kernel（WSL2、Docker Desktop 的 VM）或模擬 kernel ABI（WSL1、FreeBSD linuxulator）；跨 ISA 要模擬器（qemu-user、Rosetta）。

```
能跑 = ISA 對 + 執行檔格式對 + kernel ABI 對 + 載入器找得到 + libc ABI 對（版本 ≥）
```

## 6. Linux 是唯一承諾 syscall ABI 穩定的 kernel

| | Linux | macOS | Windows |
|---|---|---|---|
| 穩定契約在哪一層 | kernel syscall | libSystem（user space） | kernel32 / ntdll（user space） |
| 能繞過 libc 直接發 syscall 嗎 | 能 | 不該（會壞） | 不該（會壞） |
| 完全靜態 binary | 可以（musl） | 不行 | 不行 |
| 跨發行版 / 跨版本靠什麼 | 靜態連結、或 glibc 符號版本 | 系統自帶 libSystem 向後相容 | 系統自帶 kernel32 向後相容 |

- Linux 的規矩「不破壞 userspace」= syscall 號碼與語意永遠向後相容 → 靜態 binary 十年不壞。
- Apple 明說 syscall 介面私有、每版可改；Windows syscall 號碼每個 build 都變。
- 後果：Go 在 Linux 純靜態不碰 libc；在 macOS 原本直接發 syscall、被弄壞過，Go 1.12 起一律呼叫 libSystem。Rust 在 macOS 永遠動態連 libSystem，沒有「靜態 macOS binary」這種 target。
- 「ABI 不同」要拆成三個：CPU ABI（Apple Silicon 不能跑 Intel binary）、kernel ABI（macOS 不能跑 Linux binary）、libc ABI（Alpine）。

## 7. 事件解剖：症狀在 ④，根因在 ⑤

- 症狀（④）與根因（⑤）的關係：**因為 libc 實作不同（musl ≠ glibc），所以 Alpine 沒有也沒理由有 glibc 的載入器。** 硬把 Debian 的載入器複製進去，④ 過了接著在 ⑤ 死。
- 動態連結的 `execve` 其實是：kernel 先載入 PT_INTERP 指的載入器、把控制權交給它，由載入器在 user mode 找 NEEDED 的 `.so`、mmap、重定位，最後跳 `_start`。載入器是 user space 程式，kernel 只負責把它放進來（接 note 05 的「exec 換映像」）。

**「放 .so 進去還是不行」的四種死法**

| 放了什麼 | 死在哪 | 錯誤長相 |
|---|---|---|
| 只放 `libc.so.6`，沒放載入器 | 還是 ④ | 一樣 `not found` |
| 載入器 + `libc.so.6`，但 binary 還 NEEDED 別的（`libm.so.6`、`libgcc_s.so.1`、2.34 前的 `libpthread.so.0` / `libdl.so.2`） | ⑤ 載入器找下一個 .so | `error while loading shared libraries: libm.so.6: cannot open shared object file` |
| 全放齊，但來源 glibc 比編譯時的舊 | ⑤ 符號版本 | `version 'GLIBC_2.34' not found (required by ./app)` |
| 全放齊且版本夠，啟動了，但 DNS / 使用者查詢壞 | 執行期 `dlopen`：glibc 的 NSS 動態載 `libnss_dns.so.2`、`libnss_files.so.2` | 靜默失敗，`ldd` 看不到 |

**全套塞齊會怎樣**：會跑，但那個 process 不是「用 Alpine 的 libc」，是「在 Alpine 的檔案系統裡跑 Debian 的 glibc」。兩套 libc 並存：Alpine 自己的 `sh` / `apk` 走 musl（PT_INTERP 指 ld-musl），你的 binary 走 glibc；**每個 process 用哪套由它自己 ELF header 的 PT_INTERP 決定**，彼此不干擾，kernel 無感。`alpine-pkg-glibc` / `gcompat` 做的就是把整個 glibc 目錄樹塞進 `/usr/glibc-compat` 再 symlink `/lib64`。

```
t0  kernel execve     PT_INTERP 存在 ✅ → mmap 載入器，跳進去
t1  ld-linux (user)   讀 NEEDED 清單：libc.so.6、libm.so.6、libgcc_s.so.1
t2  ld-linux          依 /etc/ld.so.cache、LD_LIBRARY_PATH、預設路徑逐個找 → 任一缺 → 印錯、exit
t3  ld-linux          對每個符號比對版本標籤 → binary 要 GLIBC_2.34、libc.so.6 只到 2.31 → 印錯、exit
t4  ld-linux          重定位、跑各 .so 的初始化、跳 _start → main
t5  程式               getaddrinfo() → glibc 讀 /etc/nsswitch.conf → dlopen("libnss_dns.so.2") → 沒有 → 解析失敗
```

**兩種 .so 依賴**

| 種類 | 什麼時候載入 | 誰載 | `ldd` 看得到嗎 | 缺了的錯誤 |
|---|---|---|---|---|
| NEEDED（啟動依賴） | execve 之後、`main` 之前 | 載入器 | 看得到 | `cannot open shared object file` |
| `dlopen`（執行期依賴） | 跑到某個函數時 | 該函數自己 | **看不到** | 看那個函數怎麼處理；glibc NSS 是靜默回「查不到」 |

一句話：**binary 在 Alpine 裡是「自帶一套 glibc 住在 musl 系統裡」；能跑到哪一步取決於那套 glibc 搬進來的完整度。**

## 8. 四種解法

| 解法 | 改哪一層 | 備註 |
|---|---|---|
| Rust 改 target `x86_64-unknown-linux-musl` | 編譯期：靜態連結 musl，沒有 PT_INTERP、沒有 NEEDED | 最乾淨；`ldd` 說 "not a dynamic executable"；`scratch` image 都能跑 |
| 在 Alpine 裡 build | 編譯期：連的就是 musl | 多階段 Dockerfile 常見 |
| 換 base image（`debian:slim`、`distroless/cc`） | 執行期：環境提供 glibc | 最省事，image 大 20–30 MB |
| Alpine `apk add gcompat` / `alpine-pkg-glibc` | 執行期：墊整套 glibc | hack，複雜程式會踩到 |

Go 對照：pure Go 預設靜態、不碰 libc，Alpine 沒事；開 cgo（系統 DNS、sqlite）就動態連 glibc，撞同一個問題；`CGO_ENABLED=0` 等於 musl target 那件事。

## 9. 「依賴」有四種，靜態連結只消掉一種

musl 靜態 Rust binary 的 `getaddrinfo`（resolver 程式碼整個編進 binary，執行期無 dlopen）：

```
t0  程式 (user)         getaddrinfo("example.com") → 跳到 binary 內部的 musl resolver
t1  musl resolver       open("/etc/hosts") 逐行比對 → 沒有
t2  musl resolver       open("/etc/resolv.conf") 讀 nameserver（每次重讀，不快取）
t3  musl resolver       socket(SOCK_DGRAM) → 對所有 nameserver 同時發 UDP（並行，不輪流）
t4  kernel              socket / sendto / recvfrom：走 Linux 穩定的 kernel ABI
t5  musl resolver       收第一個答案 → 組 addrinfo → 回
```

glibc 的同一個函數：先讀 `/etc/nsswitch.conf`，依 `hosts: files dns mdns` 逐個 `dlopen` NSS 外掛。musl 沒有 NSS，寫死 hosts 檔 + DNS 兩步。**同一個 POSIX API，兩家實作拖出的依賴種類與數量不同**——API 只規定「輸入主機名、輸出位址」，沒規定要不要在執行期載外掛。

| 依賴種類 | 靜態 musl binary 還有嗎 | 例子 | 缺了會怎樣 |
|---|---|---|---|
| 程式碼（ABI） | **沒有** | 載入器、libc.so、NSS 外掛 | — |
| kernel（syscall ABI） | 有 | `socket`、`openat` | 只有 Linux 承諾穩定，可放心 |
| 資料檔 | **有** | `/etc/resolv.conf`、`/etc/hosts`、`/etc/ssl/certs/*`、`/usr/share/zoneinfo` | DNS 查不到、TLS 握手失敗、時區全 UTC |
| 環境 | 有 | 網路、`$HOME`、環境變數 | 各自失敗 |

Docker 上的一正一反：`FROM scratch` + musl 靜態 binary，DNS **能用**（Docker 把 host 產生的 `/etc/resolv.conf`、`/etc/hosts` bind mount 進每個容器）；同一 binary 用 rustls 連 HTTPS **失敗**（scratch 沒有 `/etc/ssl/certs/ca-certificates.crt`）→ `COPY --from=alpine /etc/ssl/certs/`，或 `webpki-roots` 把 CA 編進 binary（把資料依賴也靜態化）。

musl resolver 歷史包袱（2026 時大多已修）：早年不處理 `search` / `domain`（1.1.13 加）、回應超過 512 bytes 不 fallback TCP（1.2.4 / 2023 加）、並行查所有 nameserver 在 split-horizon DNS 下拿錯答案。Kubernetes 上「Alpine 容器 DNS 怪怪的」來自這裡；不是 ABI 問題，是同一個 API 的兩個實作語意不同。

一句話：**靜態連結消掉「我的 bytes 和別人的 bytes 要對上」；「我的 bytes 要讀誰的檔案、走誰的 syscall」還在。前者是 ABI，後者不是。**

## 10. 陷阱清單

1. ❌【模型】用到 libc = 用到 Unix kernel 裡的 libc → ✅ libc 是 user space 共享函式庫，kernel 裡沒有 libc；每個發行版自己選實作 → 為什麼：kernel 只看到 syscall 號碼與暫存器；libc 是發行版打包的 `.so`，Debian 選 glibc、Alpine 選 musl，kernel 無感。
2. ❌【模型】藍圖六大子系統其中一個是 libc → ✅ libc 在 trap 線上方的 user space 那一行，六個框全在線下 → 為什麼：子系統是 kernel 的骨架；libc 的工作是把 C 函數翻成 syscall，站在門外。
3. ❌【模型】libc 版本不同是因為底層串接的 driver 不同 → ✅ libc 從不碰 driver；差異在 user space 內部（binary ↔ `libc.so` 的 ABI） → 為什麼：libc 只透過 syscall 跟 kernel 講話，driver 在 kernel 下兩層；musl 與 glibc 的差別全是符號名、struct 佈局、載入器路徑這些 user space 的事。
4. ❌【模型】符合 POSIX 規格 = 可以互換 → ✅ 規格只管 API（原始碼層），不管 ABI（二進位層）；兩個都符合 POSIX 的 libc 可以 ABI 完全不相容 → 為什麼：編譯期對的是 API，執行期對的是 ABI；POSIX 沒規定符號版本、struct offset、載入器路徑。
5. ❌【模型】用什麼 OS compile 就只支援那個 OS runtime → ✅ target 決定產物不是 host（可 cross-compile）；同 OS 不同 libc 也不能跑；靜態 musl 跨發行版能跑。單位是 target triple → 為什麼：編譯器依 target triple 決定 ISA、執行檔格式、libc；host 只是跑編譯器的機器。
6. ❌【模型】Ubuntu 編的可以跑在「相容的 Unix」上 → ✅ 範圍是 x86_64 + Linux + glibc 版本 ≥；FreeBSD 也是 Unix 照樣死在 ③ → 為什麼：能跑要五層都對；FreeBSD 的 syscall 號碼不同，「Unix」這個血統在 ABI 層沒有意義。
7. ❌【模型】ABI 是硬體串接、kernel 有 ABI 是因為它操作硬體 → ✅ ABI 出現在每個「分開編譯、執行期碰面」的邊界，兩造多半都是軟體；只有 ISA 的對手是硬體 → 為什麼：binary ↔ `libc.so`、libc ↔ kernel 的兩造都是軟體；ABI 的存在理由是「兩邊都沒有原始碼了」，不是「有一邊是硬體」。
8. ❌【用詞】ABI 是「串接」這個動作 → ✅ ABI 是規格；kernel 執行 binary 時不是在串 ABI，是雙方各自假設對方遵守；違約多半靜默 → 為什麼：沒有任何元件在執行期「做 ABI」；只有 magic、`e_machine`、PT_INTERP、符號版本這幾個有標籤的地方會檢查，其餘不查。
9. ❌【模型】Alpine 那次 binary 有啟動、只是連結檔找不到 → ✅ 沒有啟動；死在 kernel `execve` 找 PT_INTERP（④），程式零指令執行；根因在 ⑤（musl ≠ glibc） → 為什麼：`execve` 在 kernel 內先 open 載入器，ENOENT 就整個失敗回 shell；shell 印的「not found」指的是載入器。
10. ❌【模型】把 Alpine 的 musl「換成」glibc → ✅ 是在 musl 旁邊多放一套 glibc；Alpine 自己的工具仍走 musl；每個 process 用哪套由自己的 PT_INTERP 決定 → 為什麼：libc 不是系統層級的開關，是每個 ELF 自己寫死的載入器路徑 + NEEDED；兩套並存 kernel 無感。
11. ❌【模型】放載入器 + `libc.so.6` 就補齊了 → ✅ 還有其他 NEEDED、符號版本、以及 `ldd` 看不到的執行期 dlopen（NSS 外掛）；補的是「一整套 libc」不是兩個檔案 → 為什麼：NEEDED 常不只 `libc.so.6`；符號有版本標籤；glibc 的 `getaddrinfo` 在執行期 `dlopen` 外掛，`ldd` 只列啟動依賴。
12. ❌【模型】`getaddrinfo` 是 glibc 獨有、換成 glibc 才能串 → ✅ 它是 POSIX API，兩家都有；挑它是因為它是「同 API、兩家執行期依賴差最多」的放大鏡 → 為什麼：API 只規定輸入主機名、輸出位址；glibc 用 NSS 載外掛、musl 寫死 hosts + DNS，同一個函數拖出的依賴種類不同。

## 11. 腦筋急轉彎：用新模型推

> **補強**：這十題是這條線留下的未答題，答案是入庫時整理者用上面的模型推的，未經導師確認。經 /校正 確認後移除標記、併入對應章節；推錯的改掉並記進 §10。

1. **靜態 musl 的 Rust binary 在 macOS 能跑嗎？死在哪？**
   不能，死在 kernel、第 ② 層。XNU 的 `execve` 依序試 Mach-O、fat、`#!` 三種 image activator，拿到 ELF magic 一個都不認 → `ENOEXEC`，zsh 印 `exec format error`。就算格式過了也會死在 ③：XNU 的 syscall 號碼與 Linux 完全不同（BSD 類在號碼高位帶 `0x2000000` 前綴）。載入器與 libc 兩層根本輪不到——靜態 musl 本來就沒有 PT_INTERP。
2. **靜態 musl binary 沒有 NEEDED、沒有 PT_INTERP，還有 ABI 依賴嗎？**
   有。§3 那三份契約裡它只消掉第一份（binary ↔ `libc.so`：libc 已經在同一次編譯裡，邊界消失），剩下 libc ↔ kernel 的 syscall ABI 與機械碼 ↔ CPU 的 ISA，外加 kernel 要認得 ELF 這個容器。「感覺沒有依賴」是因為剩下的兩份剛好是 Linux 與 CPU 都承諾穩定的。
3. **glibc 2.34 把 `libpthread` 併進 `libc.so.6`，兩個方向各怎麼樣？**
   2020 編的 binary 在 2024 Debian 能跑，靠兩件事：glibc 2.34 起仍出貨一個**空殼** `libpthread.so.0`（沒有程式碼，但保留 `GLIBC_2.2.5` 這些 version node，載入器比對版本才過得了），而 `libc.so.6` 同時匯出 `pthread_create@GLIBC_2.2.5`（compat）與 `pthread_create@@GLIBC_2.34`（default）；舊 binary 要的是前者，載入器在全域搜尋範圍裡從 `libc.so.6` 解到。反過來 2024 編的放 2020 Debian：binary 只 NEEDED `libc.so.6`、符號要 `GLIBC_2.34`，載入器（`ld-linux`，user mode）在 §7 時間軸的 t3 比對 version node 失敗，印 `./app: /lib/x86_64-linux-gnu/libc.so.6: version 'GLIBC_2.34' not found (required by ./app)` 然後 exit——印錯的是載入器，不是 kernel 也不是 shell，程式的 `_start` 沒跑到。
4. **Docker 在 macOS 跑 ELF + glibc 的 Linux image，Docker Desktop 在哪一層動了手腳？**
   不在五層裡的任何一層，是把**檢查者換掉**：Docker Desktop 開一台 Linux VM（Virtualization.framework，舊版 HyperKit），容器的每個 `execve` 都發生在 VM 裡那顆 Linux kernel 上，XNU 從頭到尾沒看到那個 ELF。檔案系統（virtiofs）與網路（VM NAT）是另外接管線進去。Apple Silicon 上跑 amd64 image 時，VM 裡再用 `binfmt_misc` 把 ① 交給 qemu-user 或 Rosetta for Linux 代打——那才是在五層之一動手腳。
5. **2012 年編的靜態 binary 與 2012 年編的 glibc 動態 binary，哪個今天更可能還能跑？**
   靜態的，但兩個大概率都還能跑。靜態只依賴 ① ISA 與 ③ kernel syscall ABI，兩者都有明確承諾（kernel 連 2011 年前 binary 用的 vsyscall 頁都還留著模擬，就是為了這個承諾）。動態的多依賴 ④ 載入器路徑（`/lib64/ld-linux-x86-64.so.2` 十幾年沒變）與 ⑤ glibc 的向後相容（舊 version node 永遠保留，2012 的 binary 要 `GLIBC_2.15`，今天的 `libc.so.6` 仍有）；多兩層就多兩個失效點。一個反例要記住：**靜態 glibc** binary 的 `getaddrinfo` 仍會 `dlopen` NSS 外掛、且要求外掛與自己同版本，2012 年的靜態 glibc binary 今天 DNS 大概率壞；靜態 musl 沒這個問題（§9）。
6. **FreeBSD 的 Linux 相容層補的是 ③，那 ④⑤ 從哪來？跟 WSL1 是同一種做法嗎？**
   ④⑤ 是真的 glibc，不是模擬：從一份真的 Linux 使用者空間（`linux_base-*` package，整包 CentOS / Rocky 的 rootfs）裝到 `/compat/linux`。linuxulator 靠 ELF 的 OSABI / `.note.ABI-tag` 認出 Linux binary、換成 Linux 的 syscall 表，並把它的路徑查找先加上 `/compat/linux` 前綴，所以 PT_INTERP 的 `/lib64/ld-linux-x86-64.so.2` 實際打開的是 `/compat/linux/lib64/ld-linux-x86-64.so.2`。WSL1 是同一種做法：NT kernel 裡的 `lxcore.sys` 實作 Linux syscall ABI（③），④⑤ 來自真的發行版 rootfs。兩者都是「kernel 補 ③、user space 搬真貨」；WSL2 則是「補一顆 kernel」（note 05 §10）。
7. **2024 Ubuntu 編的 gnu-target binary 放 2020 Ubuntu 死在 ⑤。不換 target、不改機器，只改 build 設定能不能過？**
   能。⑤ 錯誤訊息裡的版本號不是 Rust 決定的，是**連結那一步**決定的：linker 把每個符號引用綁到連結時機器上 `libc.so.6` 的 default version（`@@` 那個）。所以只要連結時看到的是舊 glibc 的符號表就過：用 zig 當 linker（`cargo zigbuild --target x86_64-unknown-linux-gnu.2.31`，自帶各版 glibc stub）、或 `-C link-arg=--sysroot=` 指向舊 glibc 的 sysroot、或在舊發行版的容器裡編（manylinux 的做法）。Rust std 本身只要求 glibc ≥ 2.17，不是瓶頸。
8. **Linux 和 macOS 都是 x86-64、System V 呼叫慣例，把 Linux 的 `.o` 拿去 macOS 連結，差在哪一層？**
   差在「`.o` ↔ linker」這條邊界，§3 的表少列了一行：連結時也有一份契約。函數內部的 bytes 兩邊真的互通（同 ISA、同呼叫慣例），但 macOS 的 `ld` 只吃 Mach-O object，不吃 ELF relocatable；就算格式轉了，C 符號在 Mach-O 帶前導底線（`_main`、`_printf`，ELF 不帶）、section 名不同（ELF `.text` / `.rodata` / `.data` / `.bss` 對 Mach-O `__TEXT,__text` / `__TEXT,__cstring` / `__DATA,__data` / `__DATA,__bss`，note 04 的 ELF 圖是 Linux 版）、重定位類型不同（`R_X86_64_PLT32` 對 `X86_64_RELOC_BRANCH`）。再往上才是 libc ABI（libSystem 的 struct 佈局、errno 位置）與 syscall ABI。
9. **Rust 沒有穩定 ABI，但 `std` 是預編好的。矛盾嗎？**
   不矛盾。「不穩定」說的是**跨 rustc 版本**不承諾；同一個 rustc 版本之內，layout 演算法與呼叫慣例是確定的。預編的 `std`（sysroot 裡的 `.rlib`）是跟那顆 rustc 同版本、同 target 一起編出來、由 rustup 當一個 toolchain 一起裝的。保證機制有兩道：`.rlib` 的 metadata 記著編它的 rustc 版本，對不上直接拒絕（`E0514: found crate 'std' compiled by an incompatible version of rustc`）；而 `std` 裡所有 generic 程式碼是以 MIR 存在 rlib 裡、在你的 crate 單態化，layout 由你手上這顆 rustc 決定，預編的機械碼只有非 generic 的部分。
10. **glibc 改了 `struct stat` 欄位順序，但 syscall 號碼、符號名、版本標籤都沒動。舊 binary 能啟動嗎？**
    能啟動——載入器只比對符號名與 version node，struct 佈局不在它的檢查範圍。出事在第一次 `stat()` 回來讀欄位時：binary 用舊 offset 讀 `st_size`，拿到的是別的欄位（`st_ino`、`st_mtime` 之類），不崩、靜默錯：檔案大小變成怪數字，拿它去配記憶體或當 `read` 長度才間接崩。「有標籤的檢查」只守名字層，不守佈局層；glibc 自己也知道，所以**佈局變更一律靠換符號版本來貼標籤**（2.33 之前 `stat` 其實是 `__xstat(ver, path, buf)`，那個 `ver` 參數就是 `struct stat` 的佈局版本號；2.33 起改成真正匯出的 `stat@@GLIBC_2.33`）。題目假設的「沒動標籤就改佈局」在 glibc 是被規矩禁止的事，不是被機制擋住的事。
