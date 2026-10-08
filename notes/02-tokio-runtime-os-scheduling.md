# Note 2 — Tokio Runtime 與 OS 排程：從 `.await` 挖到硬體中斷

> 這份筆記記錄的是「tokio 多執行緒 runtime 到底怎麼運作」這條線，從 user mode 的 task 一路往下追到 kernel、CPU、中斷。
> 核心收穫不是 tokio API，而是**兩層排程器（kernel 排 thread、tokio 排 task）各自怎麼動、在哪裡交界**。

---

## 0. 一句話版本

> **沒有任何東西能自己從「沒在跑」變成「在跑」。能動性永遠來自正在執行的東西，追到底是硬體中斷。**
> kernel 用這條原則排 thread；tokio 在 user mode 用同一條原則排 task。兩層之間靠 syscall 進出。

---

## 1. 分層模型（整份筆記的骨架）

```
┌─────────────────────────────────────────────┐
│ rustc / std      async fn → 狀態機；Future / Poll / Waker 介面  │
├─────────────────────────────────────────────┤
│ tokio (crate)    排程 task：worker loop、佇列、driver、timer wheel │  ← user mode
├─────────────────────────────────────────────┤  ← syscall / iret 是交界
│ kernel           排程 thread：run queue、wait queue、switch_to  │  ← kernel mode
│                  epoll、futex、socket buffer、page cache、network driver │
├─────────────────────────────────────────────┤
│ 硬體             core 的 fetch-decode-execute、中斷、timer、網卡  │
└─────────────────────────────────────────────┘
```

**kernel 排的是 thread，從頭到尾不知道 task 存在。task 是 tokio 在 user mode 自己玩的遊戲。**

---

## 2. 硬體層：CPU 與 thread 的本質

### core 只做一件事
```
loop { 指令 = 記憶體[PC]; 解碼; 執行; PC += 長度 }
```
core 沒有「程式」「thread」「kernel」的概念，只有 PC 指到哪就跑哪。

### thread = 暫存器快照 + stack
- 一條 thread 在硬體層面只是：一組暫存器的值（PC、SP、RAX…）+ 一塊 stack
- kernel 用 `task_struct` 存這些；誰的快照被載入 CPU 暫存器，誰就在「跑」
- 每條 thread 有**兩個 stack**：user stack（程式看得到）、kernel stack（16KB，程式看不到，進 kernel mode 時 SP 切過去）

### 三種狀態與 wait queue
```
Running  — 在某顆 core 上
Runnable — 在 run queue 排隊等 CPU
Blocked  — 掛在某個 wait queue 上，scheduler 不會考慮它
```
Blocked 的 thread **沒有任何能動性**。喚醒它只有兩種來源：
1. 另一條 Running thread 的 syscall（`futex_wake`、`write(eventfd)`）
2. 硬體中斷（timer 到期、網卡收包）

### interrupt：打破迴圈的唯一機制
- CPU 每條指令執行完會檢查中斷訊號；有 → 硬體**強制**把 PC 轉到 kernel 的中斷處理函式，存下原本的 PC/SP/flags
- syscall = 軟體主動觸發的中斷（`syscall` 指令），效果相同
- `iret` / `sysret` = 反操作：從 kernel stack 把 PC/SP/flags pop 回來，特權等級回 ring 3，繼續跑 user code
- **kernel 平常根本沒在跑。** 它是一堆躺在記憶體裡的函式，只有中斷/syscall 把 PC 丟給它才跑一下

### scheduler 就是一個函式
```c
timer_interrupt() { 處理到期 hrtimer（wake_up）; 時間片用完? schedule(); iret }
schedule()        { next = 挑 run queue 裡該跑的; switch_to(current, next) }
switch_to         { 存 current 暫存器 → 載 next 暫存器 → ret }   // ret 從「換過的 stack」pop PC
```
context switch 的全部 = 存一組暫存器、載另一組、`ret`。被切走的 thread 完全不知道中間過了多久。

### 多核 / Hyperthreading
| | 複製了什麼 | 共用什麼 | 純算術加速 | 記憶體密集加速 |
|---|---|---|---|---|
| 2 hyperthread（GCP 預設 vCPU） | 暫存器、PC | 執行單元、L1/L2、分支預測 | ~1.0–1.2× | ~1.3–1.5× |
| 2 物理 core | 以上 + 執行單元 + L1/L2 | L3、記憶體匯流排 | ~2× | ~1.3–1.7× |

- 每顆 core 各自跑 fetch-decode-execute、各自收 timer 中斷、各自跑 `schedule()`、各自有 run queue；core 間靠 IPI 互敲
- HT 的本意是「第二條 thread 撿第一條的執行單元空檔」，純算術工作沒空檔可撿
- tokio 的 `available_parallelism()` 數的是邏輯 core

---

## 3. user mode / kernel mode 是 thread 的狀態，不是程式的一部分

- 把 thread 想成一個人，kernel 想成政府機關：平常在自己公司上班（user mode）；要辦事走進機關（kernel mode）；辦完走出來
- **政府機關不是公司的一部分。** 全系統只有一份 kernel 程式碼，所有 process 的所有 thread 共用
- 「kernel stack」是機關為每個來辦事的人各留一個櫃檯，屬於機關
- 中斷處理函式跑在 **interrupt context**，不屬於任何 process / thread；當時 core 在跑誰都會被搶走，做完 `iret` 還回去
- fd 屬於 **process**（所有 thread 共用一張 fd 表），kernel 裡沒有「fd → tid」的對應

---

## 4. Tokio runtime 的結構

### 元件
```
main thread      — Runtime::new() 建 N 條 worker；block_on(main future) 在主執行緒跑，不是 worker
worker × N       — 各自跑同一份 worker_loop，各自獨立，沒有管理者
  ├ LIFO slot    — 剛被喚醒的 task 插隊（cache locality）
  ├ local queue  — 固定 256 格
  └ steal        — 沒事時偷別人 local queue 的一半
global queue     — 從非 worker thread spawn / wake 的 task 進這裡
driver (struct)  — 跟 kernel 講話的那一層：epfd + Slab<ScheduledIo>（socket 登記表）+ timer wheel + eventfd
idle_set         — user mode 的「誰在睡」名單，決定 notify 誰
blocking pool    — spawn_blocking 用，上限 512 條，重用、閒置回收
```

### worker loop（每條 worker 從生到死跑的程式碼）
```rust
loop {
    if let Some(t) = lifo_slot.take()     { run(t); continue; }
    if let Some(t) = local_queue.pop()    { run(t); continue; }
    if let Some(t) = global_queue.pop()   { run(t); continue; }   // 每 31 tick 強制來一次
    if let Some(t) = steal_from_others()  { run(t); continue; }
    // ── 沒工作 ──
    idle_set.insert(me);
    if let Some(driver) = DRIVER.try_lock() {
        driver.epoll_wait(timeout = timer_wheel.next_expiration());  // 睡法 A：盯 fd + timer
        driver.process_io_events();    // 醒來無條件做：翻名片、wake()
        driver.timer_wheel.process(now);
    } else {
        condvar.wait();                // 睡法 B：只能被同伴 notify
    }
    idle_set.remove(me);
}
// run(t) 內部：每 61 次 poll → try_lock driver → epoll_wait(timeout=0) 看一眼 + 處理 timer
```

### 關鍵規則
- **沒有派工。** 從「全部 miss」到「進 epoll_wait」只是 `if` 鏈走完後程式碼的下一行，是 worker 自己走到的
- **恰好一條 worker 在 `epoll_wait`，其餘 idle 的在 condvar。** `try_lock` 不是在選人，是在決定「誰用哪種姿勢睡」；保證只要有任何 worker idle，就有一條盯著 fd（kernel 本身允許多條 epoll_wait，驚群問題；tokio 選擇在 user mode 鎖成一條）
- **worker 之間唯一的互動**：`notify_one` / 寫 eventfd（叫醒，不派任務）和 steal（偷工作）。被叫醒的 worker 自己回 loop 找工作，可能白醒一趟
- **task 之間沒有順序保證。** global queue 是 FIFO，但多 worker 競爭取出 + 喚醒進 LIFO slot，順序不可依賴
- 全 busy 時：事件躺在 kernel 的 ready list，最多延遲 61 次 poll；但 61 是 poll 次數不是時間，task 不讓出 → 永遠數不到

---

## 5. Task 與 Waker

### task 是什麼
- `async fn` 被 rustc 編譯成一個**狀態機 struct**，`.await` 前後要活的區域變數全進欄位
- `spawn` 時放到 heap；**沒有 stack、沒有暫存器快照**，幾百 bytes
- `Pending` 之後 **task 不在任何佇列裡**，只是一塊沒人持有執行權的記憶體（在佇列裡就會變 busy loop）

### Waker = 一張寫著 task 地址的名片
- **它自己不會動。誰手上有名片、誰就可以喊 `wake()`；喊的那一瞬間是喊的那條 thread 在執行。**
- `wake()` 做兩件事：① schedule（推進佇列）② notify（必要時叫醒一個 idle worker）
  - 從 worker 內呼叫 → 推進自己的 LIFO slot
  - 從非 worker thread 呼叫 → global queue + `notify_one` / 寫 eventfd
- 名片放在哪、誰來翻，依 future 類型而定：

| future | 名片放在 | 誰翻出來喊 wake() |
|---|---|---|
| `tokio::time::sleep` | timer wheel（user mode 資料結構，kernel 不知道它存在） | 拿 driver 的 worker，在 park 前後 / 每 61 tick 翻到期的桶 |
| socket read | driver 的 `Slab<ScheduledIo>`（epoll token = slab 索引） | 從 `epoll_wait` 醒來的那條 worker，只處理返回的那幾個 token |
| `mpsc` channel | channel 的 waiter list（純 user mode，無 syscall） | sender 在 `send()` 時順手 |
| `spawn_blocking` | JoinHandle | blocking pool 的那條 thread 跑完閉包時 |

- **「等事件的 thread」和「處理事件的 thread」是同一條**：誰睡在 epoll_wait 裡，誰醒來就處理。沒有人「決定」誰去 wake
- 不是每條 thread 都去 scan；也不是 scan——`epoll_wait` 返回「有事的 fd 列表」，O(有事的數量)

### task vs thread
| | thread（kernel 排） | task（tokio 排） |
|---|---|---|
| 本體 | task_struct + 暫存器快照 + 16KB kernel stack + 8MB 虛擬 user stack | 一個 struct |
| 切換 | 中斷 → ring 0 → switch_to → iret | `poll()` 回 `Pending`，呼叫下一個 struct 的 `poll()`，純 user mode 函式呼叫 |
| 成本 | ~1–2 µs | ~幾十 ns |
| 可打斷點 | 任何指令之間（preemptive） | 只有 `.await`（cooperative） |

### `.await` 展開
```rust
loop {
    match fut.poll(cx) {
        Poll::Ready(v) => break v,
        Poll::Pending  => return Poll::Pending,   // 整個狀態機停在這點，下次 poll 從 loop 再來
    }
}
```
分工：展開是 rustc；`poll_read` 裡的 syscall / 登記格是 tokio；`Future` / `Poll` / `Waker` 是 std。

---

## 6. 思維鏈：`socket.read().await` 完整時間軸

場景：2 條 worker（W1、W2），task T 執行 `let n = socket.read(&mut buf).await?;`，沒有其他 task。
每一行標明：**哪條 thread、哪個 mode、跑誰的程式碼**。

```
時間  執行者              mode       發生什麼
──────────────────────────────────────────────────────────────────────
t0    W1 (core 0)        user       tokio：從佇列拿出 T，呼叫 T.poll()
t1    W1                 user       你的 async fn 走到 socket.read().await → tokio TcpStream::poll_read
t2    W1                 user→K     執行 `syscall` 指令（read，socket 是 non-blocking）
t3    W1                 kernel     kernel socket read：receive buffer 空 → 回 EWOULDBLOCK
t4    W1                 K→user     iret
t5    W1                 user       tokio：把 T 的 waker 塞進 socket 的登記格（ScheduledIo）
                                    → poll_read 回 Pending → T.poll() 回 Pending
                                    → T 從此不在任何佇列，只有登記格裡有它的名片
t6    W1                 user       worker loop：LIFO 空、local 空、global 空、偷 W2 也空
t7    W1                 user       tokio：idle_set.insert(W1)；try_lock driver 成功
t8    W1                 user→K     epoll_wait(epfd, timeout = -1 ∞，因為 timer wheel 是空的)
t9    W1                 kernel     kernel：W1 掛到 epoll 實例的 wait queue，標 Blocked
                                    scheduler：switch_to(W1 → core 0 的 idle thread)
      ─── W1 不在任何 core 上，PC 停在 kernel 的 epoll_wait 內部 ───

t10   W2 (core 1)        user       沒事做 → idle_set.insert → try_lock 失敗（W1 拿著）
                                    → condvar.wait() → futex_wait → Blocked
      ─── 你的 process 零條 thread 在跑。core 可能 hlt，或跑別的 process ───

...   不知道多久（1ms 或 1 小時）...

t20   網卡硬體            —          封包到達，發硬體中斷
t21   core 1，無 thread   kernel     【interrupt context】硬體把 PC 強制轉到 kernel 中斷處理函式
                                    （當時 core 1 在跑誰都一樣，被搶走）
                                    kernel network driver：解析封包
                                    → 用 (IP, port) 查到 struct socket → 封包存成 sk_buff 掛進
                                      receive queue（kernel heap）→ 標可讀
                                    → 呼叫 socket wait queue 上的 callback
                                      （t7 之前 epoll_ctl(ADD) 時 epoll 掛上去的）
                                    → callback：把這個 fd 放進 epoll 的 ready list
                                    → wake_up(epoll 實例的 wait queue)：此刻掛著的是 W1 → 標 Runnable
                                    → 放進 core 0 的 run queue，core 0 在 hlt → 發 IPI 敲醒
t22   core 1              kernel     iret，把 core 1 還給被打斷的那個誰
      ─── 到這裡為止，你 process 的任何 thread 都還沒執行過一條指令 ───
      ─── kernel 全程沒問過「這是哪個 process」，只是順著物件上掛的指標走 ───

t23   core 0              kernel     被 IPI 敲醒 → schedule() → 挑到 W1 → switch_to
t24   W1 (core 0)        kernel     從 t9 停下的位置繼續：epoll_wait 內部
                                    → 把 ready list（1 個 fd）寫進 user 傳進來的 events 陣列
                                    → 回傳 1
t25   W1                 K→user     iret
t26   W1                 user       tokio：idle_set.remove(W1)
                                    driver.process_io_events()：遍歷 events（只有 1 個）
                                    → 用 token 查 slab → 拿到 T 的名片 → waker.wake()
                                    → worker 在呼叫 → T 進 W1 的 LIFO slot
t27   W1                 user       worker loop：LIFO slot 有 T → T.poll()
t28   W1                 user       poll_read 再試一次 read
t29   W1                 user→K→user syscall read：沿 sk_buff list copy_to_user 到 buf，最多 buf.len()
                                    → 回 n；被複製走的 sk_buff 釋放，TCP window 變大
t30   W1                 user       Poll::Ready(n) → 你的 async fn 從 .await 下一行繼續
```

### 這條鏈的四個要點
1. **t5：Pending ≠ 在佇列。** 名片在登記格，task 在 heap，兩者分離
2. **t9 / t10：都是 parking。** 差別只是睡在 `epoll_wait`（能被 fd 事件叫醒）還是 `futex_wait`（只能被同伴叫醒）
3. **t21：全部是 OS 的事。** 中斷不屬於任何 thread；kernel 不找人，只對 wait queue 執行 `wake_up`，掛著誰就叫誰。換成 W2 睡在那裡就叫 W2
4. **t26：wake() 是 W1 自己喊的。** 因為它就是從 `epoll_wait` 帶著事件列表回來的那條；「等事件」和「處理事件」是同一條 thread

### 兩次「掛接」是整個機制的根
```
[socket] ──epoll_ctl(ADD)，建 socket 時掛一次──► [epoll 實例] ──epoll_wait，每次睡都重掛──► [thread]
```
socket 認得 epoll 實例，不認得 thread；epoll 實例認得「此刻睡在上面的 thread」。

---

## 7. epoll 的範圍與 syscall 分類

### epoll 只做 readiness
- 回報「哪些 fd 現在可讀/可寫/出錯」，**不幫你讀寫**，只保證「現在去 read 不會卡」
- 能用的 fd：socket、pipe、eventfd、timerfd、signalfd、tty
- **一般磁碟檔案不能用**：kernel 視檔案為永遠就緒，`epoll_wait` 立刻返回，但後續 `read` 可能等磁碟幾毫秒 → 卡死 worker。所以 `tokio::fs` 底層是 `spawn_blocking`
- completion 模型（資料已在你 buffer 裡）是 `io_uring`，Linux 下一代 I/O，🐊 階段

### syscall 不用背，按資源分類
| 類別 | 代表 | Rust 對應 |
|---|---|---|
| fd / I/O | `open` `read` `write` `socket` `accept` | `File` `TcpStream` |
| 等 fd | `epoll_*` `poll` `io_uring_*` | tokio driver |
| process / thread | `clone` `execve` `waitpid` | `thread::spawn` `Command` |
| 記憶體 | `mmap` `brk` | allocator、`Vec` 擴容 |
| 同步 | `futex` | `Mutex` `Condvar` `park`、channel 內部 |
| 時間 | `nanosleep` `clock_gettime` | `thread::sleep` `Instant` |

會 Blocked 的只有：等 fd 那組、`futex`、`nanosleep`、`waitpid`、blocking 的 `read`/`accept`。
工具：`strace -f cargo run` 看程式發出的每個 syscall。

---

## 8. 寫檔：可見性 vs 持久性

```
write() 返回成功 = kernel 收下，資料在 page cache（kernel heap）  → 可見性（其他 process 立刻讀得到）
         ↓ kernel 背景回寫磁碟（幾秒～30 秒）；期間斷電 = 資料消失
fsync() 返回     = 真的落盤                                      → 持久性
```
- `write()` 多數時候幾微秒，真正慢的是 `fsync`（毫秒級）和 page cache 滿時的回寫。「寫檔不能放 worker」的理由是**無法預測何時會慢**，不是「一定慢」
- socket receive buffer 同樣在 kernel heap（sk_buff 鏈），有上限（`tcp_rmem`），滿了 TCP 回報 window=0 → sender 停 → **backpressure 是 TCP 內建的**，你的責任是別在 user space 造一個無上限 buffer 把它破壞掉
- `read()` 一次把 buffer 裡有的、不超過 `buf.len()` 的資料複製過來；可能比要的少 → 要 loop；回 0 = EOF

### 三種做法
| 做法 | 適用 | 問題 |
|---|---|---|
| 直接 `std::fs` 在 worker 上 | 不要 | blocking syscall 卡住 worker 和它佇列裡的所有 task |
| `spawn_blocking` / `tokio::fs` | 中低頻 | 每次跨 thread + wake 有固定成本；高頻時並發不受控、順序無保證 |
| 專職 writer thread + `mpsc` channel | 高頻、有序、長期 | 要自己處理 flush、關閉、批次 fsync 的丟失窗口 |

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10_000);
std::thread::spawn(move || {
    let mut file = File::create("log.txt").unwrap();
    while let Some(line) = rx.blocking_recv() { writeln!(file, "{line}").unwrap(); }  // 沒資料睡 futex
});
tx.send(line).await?;   // task 裡：幾十 ns，channel 滿了自然 backpressure
```

---

## 9. 陷阱清單：錯在哪、正解是什麼、為什麼

### 🔴 必改

**1. async 裡放 blocking 操作**
- ❌ `std::thread::sleep(...)`、純 CPU 長迴圈、`std::fs::read(...)`、同步 DB client
- ✅ `tokio::time::sleep(...).await`；CPU 工作和同步 I/O 用 `spawn_blocking`；高頻寫入用專職 thread + channel
- 為什麼：worker 卡在 `poll()` 裡出不來，61 tick 永遠數不到，它佇列裡的所有 task、timer、I/O 事件一起餓死。tokio 的 task 是 cooperative，不讓出就沒人能搶

**2. 以為 `Pending` 的 task 在佇列裡排隊**
- ❌ 「task 等待時會被放回佇列等下次 poll」
- ✅ `Pending` 的 task 不在任何佇列，只有它的 waker 躺在某個 list 裡（timer wheel / ScheduledIo / channel waiter list）；被 `wake()` 才進佇列
- 為什麼：在佇列裡就會被反覆 poll = busy loop。「不在佇列」正是 task 不耗 CPU 的原因

**3. 依賴 task 之間的執行順序**
- ❌ 連續 `spawn` 四個 task 就假設 1 先於 2
- ✅ 要順序：用 `.await` 串起來、用 channel 傳遞、或 `JoinSet` 收集；要互斥：`tokio::sync::Mutex`
- 為什麼：多 worker 競爭取出 + 喚醒進 LIFO slot 插隊 + work-stealing，順序完全不可預測

**4. 以為有一個「tokio 管理者」在派工**
- ❌ 「tokio 看到 W1 忙就挑一個 idle worker 去 epoll_wait」
- ✅ 沒有管理者。每條 worker 各自自轉，沒事就自己走到睡覺那段程式碼；`try_lock` 只決定「誰用哪種姿勢睡」；worker 間唯一互動是 notify（叫醒，不派工）和 steal
- 為什麼：中心化派工會成為 bottleneck；去中心化 + 共用佇列 + 偷工作才能 scale

### 🟡 建議

**5. 兩個 driver 混成一個**
- ❌ 「網卡中斷由 tokio driver 處理」
- ✅ kernel network driver = OS 程式碼，跑在 interrupt context，不屬於任何 thread；tokio driver = user mode struct（epfd + slab + timer wheel），由拿到 lock 的 worker 使用
- 為什麼：角色類比才借同一個名字，一個在 kernel 一個在 crate

**6. user mode 和 kernel mode 的東西混放**
- ❌ 「timer wheel 是 kernel 的」「應用程式裡有一塊 kernel」
- ✅ kernel：`epoll_wait`、`futex`、wait queue、run queue、socket buffer、page cache。user：timer wheel、ScheduledIo、idle_set、LIFO slot、local/global queue、Waker。mode 是 thread 的狀態，kernel 是全系統一份的程式碼
- 為什麼：讀原始碼時這條線就是「這段是 syscall 包裝還是純資料結構」的分界

**7. 用 epoll 等磁碟檔案**
- ❌ 以為 `tokio::fs` 跟 `tokio::net` 一樣走 epoll
- ✅ 磁碟檔案在 kernel 眼中永遠就緒，`tokio::fs` 底層是 `spawn_blocking`；真正非同步的磁碟 I/O 要 `io_uring`
- 為什麼：epoll 是 readiness 模型，只對「可能暫時沒資料」的 fd 有意義（socket、pipe、eventfd）

**8. `write()` 成功就當作落盤**
- ❌ 結算結果 `write()` 返回 Ok 就認定不會丟
- ✅ `write()` = 可見性（進 page cache）；`fsync()` = 持久性。不能丟的資料要 `fsync` 或交給 DB；可以丟的 log 批次 `write` + 定期 `fsync`，自己選丟失窗口
- 為什麼：page cache 回寫是背景的、幾秒到 30 秒後；期間斷電資料消失

**9. 用 vCPU 數估平行加速**
- ❌ 2 vCPU 開 2 thread 期待 2×
- ✅ 先 `lscpu` 看 `Thread(s) per core`；GCP e2/n2 的 2 vCPU 通常是 1 物理 core + HT，純算術 ~1.0–1.2×，記憶體密集 ~1.3–1.5×；要真 2× 開 4 vCPU 或設 `threads-per-core=1`
- 為什麼：HT 共用執行單元和 L1/L2，只能撿第一條 thread 的空檔

**10. `async move` 的 `move` 當裝飾**
- ❌ 拿掉 `move`，或靠 `clone()` 硬塞過 borrow checker
- ✅ `tokio::spawn` 要求 `Send + 'static`：task 可能活得比 spawn 它的 scope 久、可能跑在別的 thread，所以必須擁有自己用到的資料。共享用 `Arc`，可變共享用 `Arc<Mutex<T>>` 或 channel
- 為什麼：worker 會換、scope 會結束，借用無法保證存活

**11. 高頻寫入每筆都 `spawn_blocking`**
- ❌ 每秒幾千筆 log 各自 `spawn_blocking(|| write(...))`
- ✅ 一條 writer thread + `mpsc` channel；task 端 `send().await` 幾十 ns，channel 滿了自然 backpressure
- 為什麼：blocking pool 重用 thread 但每次有跨 thread + wake 成本；512 條並發寫同一檔案順序亂、搶 lock

**12. 在 user space 造無上限 buffer**
- ❌ 讀 socket 後無限 `Vec::push` 等著慢慢處理
- ✅ 用 bounded channel；讀不及就讓 `read` 慢下來，TCP window 會把壓力推回 sender
- 為什麼：backpressure 是 TCP 內建的（receive buffer 滿 → window=0 → sender 停），無上限 buffer 等於親手拆掉它
