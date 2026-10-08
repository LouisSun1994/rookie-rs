# Rust 學習筆記 01：從記憶體模型到 async runtime 與作業系統

- 記錄日期：2026-10-07
- 學習方式：概念優先（conceptual-first），先建立全貌再動手實作
- 目前定位：🥚 小白階段（尚未動手寫程式），但已概念性探索到 🦀/🦞 階段的 smart pointer、async、concurrency，以及作業系統層的機制
- 背景：熟悉 Go（GMP 排程模型）、後端 / 基礎設施、遊戲系統

---

## 0. 階段路線圖（導師提供）

| 階段 | 內容 |
|---|---|
| 🥚 小白 | 環境、cargo、變數、型別、控制流程、函式、struct、enum、match |
| 🐣 入門 | ownership、borrowing、slice、String vs &str、Vec/HashMap、Option/Result、錯誤處理、module |
| 🦀 進階 | trait 與 generics、lifetime、closure、iterator、Box/Rc/RefCell/Arc、測試 |
| 🦞 熟練 | thread/channel/Mutex、async/tokio、trait object、效能分析、serde/clap/anyhow/axum |
| 🐊 巨鱷 | unsafe/FFI、macro、記憶體佈局、零成本抽象原理、API 設計、讀 std 原始碼 |

小白階段 7 課：環境與 cargo → 變數與 mutability → 基本型別 → 控制流程 → 函式 → struct → enum 與 match → 畢業專題（CLI 小工具）。

---

## 1. 語言全景

### 核心哲學
- 沒有 GC 也記憶體安全：靠 ownership 在編譯期決定釋放時機（RAII + `Drop`）
- 零成本抽象：iterator、generics 編譯後與手寫迴圈同速
- 強型別 + 型別推導、預設不可變、沒有 null（`Option`）、沒有 exception（`Result`）、expression-based
- 一句話：編譯器囉嗦的每件事，都是別的語言會在半夜讓 production 爆掉的事

### 語法速記
- `let` 不可變、`let mut` 可變、shadowing 可換型別、`const` 必須標型別
- 純量：`i8..i128/isize`、`u8..u128/usize`、`f32/f64`、`bool`、`char`（4 bytes Unicode）
- 複合：tuple `(i32, f64)`、array `[i32; 3]`（固定長度、stack）、unit `()`
- 函式最後一行不加分號 = 回傳值；加分號變 statement，值為 `()`
- `if`/`match`/區塊都是 expression；`loop` 可 `break` 帶值；`for i in 0..5`（不含 5）、`0..=5`（含）
- struct + `impl`：`Self::new()` 是 associated function；`&self` 唯讀、`&mut self` 可改
- enum 變體可攜帶資料；`match` 必須窮舉；`if let` 處理單一情況
- 註解：`//`、`///` 文件註解、`//!` 模組文件
- 常用 macro：`println!`、`format!`、`vec!`、`dbg!`、`panic!`、`todo!`、`assert_eq!`
- 格式化：`{}` Display、`{:?}` Debug、`{:#?}` 漂亮排版、`{name}` 直接嵌變數、`{:.2}`
- 命名：`snake_case`（變數/函式）、`UpperCamelCase`（型別/trait）、`SCREAMING_SNAKE_CASE`（常數）

### 新手 knowhow
- 分號決定一切；沒有隱式型別轉換（`as` 會截斷）；`if` 只吃 `bool`；沒有 `++`
- 整數溢位：debug panic、release 繞回；用 `checked_/wrapping_/saturating_`
- 陣列越界是 panic 不是 UB；未使用變數加 `_` 前綴
- 編譯錯誤有代碼：`rustc --explain E0382`
- 每次寫完跑 `cargo fmt` 和 `cargo clippy`

---

## 2. 記憶體模型：call by value、move、Copy

### 傳參三種方式（全部在呼叫處明確寫出）
| 參數型別 | 呼叫 | 語意 | 呼叫後原變數 |
|---|---|---|---|
| `T` | `f(x)` | move（或 Copy 型別則複製） | move 後失效 |
| `&T` | `f(&x)` | 共享借用，只能讀 | 照常使用 |
| `&mut T` | `f(&mut x)` | 可變借用，可改 | 照常使用 |

- Rust 永遠是 call by value；`&x` 本身是一個指標值，被按值複製
- **move 本質**：複製 stack 上的 handle（String 是 ptr/len/cap 共 24 bytes），不複製 heap；舊變數作廢以避免 double free
- **Copy 型別**：整數、浮點、bool、char、由 Copy 組成的 tuple/array；`String`/`Vec` 不是
- 可變借用要三處同意：`let mut`、`&mut x`、`fn f(x: &mut T)`
- 新手陷阱：遇 E0382 就 `.clone()`；參數選擇原則「只讀 `&T`、要改 `&mut T`、要保存/銷毀才 `T`」

### 與其他語言比較
- C：只有傳值，指標自己傳
- C++：`void f(int& x)` 呼叫處看不出會被改
- Java/Python/JS：call by sharing，物件一律共享
- Rust：`f(x)` / `f(&x)` / `f(&mut x)` 一目了然

---

## 3. Stack 與 Heap

- 每個函式呼叫開一個 stack frame，區域變數住在裡面，`main` 無特殊待遇
- **Rust 從不偷偷把東西放 heap**：只有 `String`、`Vec`、`Box`、`HashMap`、`Rc`、`Arc` 等型別會配置 heap，且它們的 handle 仍在 stack
- 同一個 `i32` 可以在 stack（`let x = 5`）、heap（`vec![5]` 的內容、`Box::new(5)`）
- 必須放 heap 的根本原因：stack frame 大小必須編譯期已知（`Sized`）；字串內容長度未知 → heap
- stack 快的原因：移動指標即配置、函式回傳整塊收掉、cache 友善

### 所有權是一棵樹
- 每塊 heap 有明確 owner；owner 可以在 heap 上（`Vec<String>` 裡的 String handle）；樹根在 stack 或 static
- heap 跟著 **owner** 結束，而 owner 可被 move（函式回傳 `String` 時 heap 活下來）
- `Rc`/`Arc` 允許多個 owner：引用計數，歸零即釋放（確定性，非 tracing GC）；循環引用會洩漏，用 `Weak`

### Stack overflow
- 主執行緒 stack：Linux/macOS 約 8 MB、Windows 約 1 MB；`thread::spawn` 預設 2 MB（可用 `thread::Builder::stack_size()` 或 `RUST_MIN_STACK` 調）
- 真正原因：深遞迴（Rust 不保證 TCO）、大 array 放 stack（改用 `Vec`）、spawn 執行緒 stack 較小
- 爆了會有 guard page 保護，直接終止程式，不會默默覆寫記憶體
- heap 無固定上限，受虛擬位址空間、RAM+swap、`ulimit -v`、容器 cgroup 限制；配置失敗預設 abort；Linux overcommit 可能被 OOM killer 殺掉

---

## 4. Rc / Arc / Send / Sync / 'static

### Arc（Atomically Reference Counted）
- 解決：多個 task/執行緒要共享同一份資料，編譯期無法決定誰最後結束 → 執行期計數
- `Arc` 本身只是 8 bytes 指標；`Arc::clone` 只複製指標 + 計數 +1，不複製資料
- `Arc<T>` 只給共享唯讀存取（等同 `&T`），要改必須搭配 `Mutex`/`RwLock`：**Arc 管共享所有權，Mutex 管安全修改**
- 慣用寫法 `Arc::clone(&x)` 而非 `x.clone()`，表明是便宜的計數操作
- 經典模式：先 clone 再 `move` 進 task；或用區塊 `{ let x = Arc::clone(&x); move || {...} }`
- 陷阱：拿著 `std::sync::Mutex` 的鎖跨 `.await`（編譯錯誤 + 卡住）；`Arc<Mutex<>>` 當躲 borrow checker 萬用解；一把大鎖鎖整張表（改 `RwLock`/`dashmap`/channel）；循環引用；`.lock()` 回 `Result` 是因 poisoning

### Rc vs Arc
- 差別只在計數是否原子操作
- `Rc` 不是 `Send`/`Sync`：跨執行緒會因「讀-加-寫」競爭導致計數錯誤 → use-after-free / double free，所以**編譯期直接禁止**（`E0277: Rc cannot be sent between threads safely`）
- `Rc` 可用於多執行緒程式，只要它和所有 clone 不離開建立它的執行緒；單執行緒 runtime 的 `spawn_local` 可用 `Rc`
- `Rc` 的用途是「所有權結構」問題（UI 元件共享設定、圖節點），不是 non-blocking 問題
- C++ `shared_ptr` 永遠付原子成本；Rust 給便宜版+安全版，用型別系統保證不會用錯

### Send / Sync
- `Send`：所有權可移到另一條執行緒；`Sync`：可被多執行緒同時借用 `&T`
- marker trait，自動傳染：struct 有一個欄位非 Send，整個就非 Send
- 非 Send 例：`Rc`、`std::sync::MutexGuard`、原始指標
- `tokio::spawn` 要求 `Send`，因為 task 在兩個 `.await` 之間可能換 worker；是否 Send 取決於**跨越 `.await` 時手上拿著什麼**

### 'static（三個長得像的東西）
| 寫法 | 是什麼 | 意思 |
|---|---|---|
| `static X: u32 = 5;` | 關鍵字 | 全域變數 |
| `&'static T` | lifetime 標記 | reference 指向整個程式期間都存在的東西（字串字面值、vtable） |
| `T: 'static` | bound | 型別**不含會過期的借用**；`String`、`Arc<...>` 都滿足，可隨時 drop |

- `'` 開頭是 lifetime 名稱，`'static` 是唯一內建的固定名稱
- `tokio::spawn` 要求 `'static`：task 由 runtime 擁有，可能活得比呼叫函式久，不能借用區域變數 → 需要 `move` + `Arc::clone`
- 不需要 Send/'static 的情況：`.await`、`join!`（子 Future 屬於當前 task，不會單獨跨執行緒、不會活更久）；`thread::scope` 允許借用（結構化並發）

---

## 5. Closure 與 move

- `|x| x + 1`：`|...|` 是參數列表，不是 or；`||` 代表無參數
- 可捕獲外部變數（`fn` 不行）；編譯器自動選最溫和的捕獲：只讀 → `&`、修改 → `&mut`、消耗 → 取走所有權
- `move` 強制全部以所有權方式捕獲；`thread::spawn` / `tokio::spawn` 需要它（E0373）
- `move` 只決定「捕獲怎麼存」，不決定能呼叫幾次；`move` closure 仍可是 `Fn`
- `Fn`（只讀、多次）/ `FnMut`（修改、多次）/ `FnOnce`（消耗、一次）
- 本質是編譯器生成的匿名 struct，捕獲變數是欄位；不捕獲則 0 bytes，零成本
- Go/JS closure 一律參考捕獲 + GC 兜底；Rust 沒 GC 所以要明確選借用或擁有

---

## 6. async / Future / Waker

### Future
- `trait Future { type Output; fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Output>; }`
- `Poll::Ready(v)` / `Poll::Pending`；**沒有 `await` 方法**
- **契約**：回 `Pending` 前必須安排 Waker 之後被呼叫，否則永不再被 poll
- **Lazy**：`async fn` / `async {}` 只產生狀態機，不執行；要 `.await` 或 `spawn` 才動（JS Promise 建立即跑、Go `go f()` 立刻排程）
- `async fn foo() -> T` ≈ `fn foo() -> impl Future<Output = T>`；呼叫 `foo()` 得 Future，`foo().await` 才執行；`foo` 本身是 fn item 不能 await
- `.await` 是**關鍵字**（無括號），編譯器展開為「poll；Pending 則離開函式回傳 Pending、下次從此處恢復」；**不是死循環**，每圈之間執行緒去跑別的
- 一個 task 是一棵 Future 樹：根（async fn）→ 子（async fn）→ 葉（`TcpStream::read`，手寫 `impl Future`，真正跟 OS 打交道）；Pending 一路往上傳
- 三方分工：Future（描述做什麼、做到哪）、executor（呼叫 poll）、reactor（監聽 I/O、呼叫 Waker）；std 只定義介面，tokio 實作後兩者
- **取消 = drop**（不再 poll）；由此衍生 cancellation safety 議題
- `Pin`：async 狀態機可能自我參照，被 poll 後不准搬家（🐊 階段）
- Future 來源：async 語法（編譯器）、函式庫（`Sleep`、`JoinHandle`）、手寫（`Countdown` 範例：回 Pending + `cx.waker().wake_by_ref()`）
- 語法糖綁定 trait 的通則：`.await`→`Future`、`for`→`IntoIterator`、`+`→`Add`、`?`→`Try`、`[]`→`Index`、`{}`→`Display`

### Waker
- 本質是兩個指標：`data`（指向 task）+ `vtable`（`wake`/`clone`/`drop`）；是一張「寫著 task 地址的名片」
- `wake()` = 把 task 推進佇列，若有 worker 在睡就叫醒一個；幾十 ns；任何執行緒都可呼叫
- **沒有週期**：事件發生那一刻被呼叫一次。來源：reactor（I/O）、計時器、其他 task（channel send、Mutex 釋放、JoinHandle 完成）、blocking thread 做完、`yield_now` 自己
- worker 對 Pending 的 task 零責任：不記錄、不追蹤、不回頭看

### spawn / join! / await 的差別
| 寫法 | 何時跑 | 並發 | 平行 | 需要 Send+'static |
|---|---|---|---|---|
| `let f = async {}` | 不跑 | — | — | — |
| `f.await` | 立刻，當前 task 內 | ❌ | ❌ | ❌ |
| `tokio::join!(a, b)` | 立刻，當前 task 內交錯 | ✅ | ❌ | ❌ |
| `tokio::spawn(f)` | **立刻**排入佇列 | ✅ | ✅（多執行緒 runtime） | ✅ |
| `spawn_local` | 立刻 | ✅ | ❌ | 'static only |
| `spawn_blocking(closure)` | 立刻，blocking pool | ✅ | ✅ | ✅ |
| `std::thread::spawn` | 立刻建 OS 執行緒 | ✅ | ✅ | ✅ |

- **spawn 不是 lazy**：立刻提交；`JoinHandle.await` 是等結果不是啟動；丟掉 JoinHandle 不會取消（detach）
- `JoinHandle.await` 回 `Result<T, JoinError>`（task panic 或被 abort）；tokio 隔離 task panic，Go 的 goroutine panic 會殺整個程式
- **main 結束 → runtime drop → 所有 task 在下一個 `.await` 點被取消**（原因是 runtime 生命週期，不是沒 await）
- `join!` 的子 Future owner 是當前 task；spawn 的 task owner 是 runtime；`JoinHandle` 只是取件單；`JoinSet` drop 時取消內部所有 task（結構化並發）
- I/O 密集：`join!` 夠用；需獨立生命週期：`spawn`；CPU 密集：`spawn_blocking`/`rayon`

### blocking vs suspending
| | blocking（阻塞） | suspending（暫停） |
|---|---|---|
| 誰停 | 執行緒 | task |
| 執行緒能做別的嗎 | ❌ | ✅ |
| 來源 | `std::thread::sleep`、同步 I/O、長計算 | `.await` 一個 Pending 的 Future |

- A/B/C（循序 await / join! / spawn）**全部是 non-blocking**，差別在 task 內循序或交錯、能否平行
- `join!` 在 worker 上執行不會影響 main thread，也不會 block 那個 worker
- main thread 休眠（`block_on`）的原因是它負責的那棵 Future 樹整棵在等，不是因為它是 main
- 在 async 語境，「blocking」真正要問的是「有沒有佔住 worker」
- 兩個 `.await` 之間的程式碼一口氣跑完，不可被打斷（cooperative）；經驗法則：不超過 10～100 µs
- 加 `.await` 不會讓同步操作變非同步：`std::thread::sleep(...).await` 編譯錯誤（`()` 不是 Future）；要用 async 版本（`tokio::time::sleep`、`tokio::fs`、`reqwest::get`）
- `spawn_blocking`：丟到獨立 blocking pool（0～512 條，按需建立，閒置 ~10 秒回收，開始後無法 abort）；呼叫端 task 暫停、worker 自由；真正 blocking 的是 blocking thread（本來就是它的用途）；判斷標準是「操作會不會讓出執行緒」，不是「I/O 還是 CPU」；大量 CPU 計算用 `rayon` 或 `Semaphore` 限流
- `tokio::fs` 內部用 `spawn_blocking` 包同步版本
- `yield_now().await`：回 Pending + 立刻喚醒自己 → 回佇列尾（= Go `runtime.Gosched()`）；適合切長迴圈，對單次慢操作和同步 I/O 無效；tokio coop budget 只保護自家資源
- `yield` 關鍵字保留未穩定；Python generator 對應 Rust `Iterator`
- 真正危險：async 裡自寫無 `.await` 的自旋等待 `while !flag {}`

### 自旋 vs .await 的 loop
- `.await` 展開的 loop 每圈 Pending 時**離開函式**，兩圈之間可能隔秒，worker 不在裡面
- 真正的等待集中在 reactor 的一次 `epoll_wait`，同時等所有 I/O

---

## 7. Tokio runtime 結構

- `#[tokio::main]` 展開為 `Builder::new_multi_thread().enable_all().build().unwrap().block_on(async {...})`
- 三群執行緒：**main thread**（1 條，只 `block_on` 推動 main Future，不參與 work stealing）、**worker**（預設 = 邏輯 CPU 數，啟動時建好，各有本地佇列 + 全域佇列 + work stealing）、**blocking pool**（獨立，不從 worker 撥）
- 平行 = 多個 worker 同時 poll 不同 task；task 在 `.await` 後可能換 worker
- 任務的 stack 在 heap：async fn 編譯成狀態機 struct，跨越 `.await` 的變數成為欄位，spawn 時 Box 到 heap，任何 worker 可接手（stackless；Go goroutine 是 stackful）
- Future 分兩部分：程式碼（執行檔固定位址，透過 vtable 找到 poll）+ 資料（heap 狀態機）；runtime 佇列只存指標；`move` 只影響資料存的是值還是 reference
- **沒有獨立 reactor 執行緒**：閒下來的 worker 兼任拿 I/O driver 去 `epoll_wait`；其他閒置 worker 用 `futex` 睡
- **滿載時**：每 61 次 poll（`event_interval`）做一次維護，`epoll_wait(timeout=0)` 收事件；31（`global_queue_interval`）檢查全域佇列；質數避免共振；Go 排程器也是每 61 次檢查全域佇列。滿載是延遲上升不是 crash；佇列無限成長才是問題（背壓：有界 channel、Semaphore）
- main thread 角色：發起者 + 守門人（初始化、接客迴圈 spawn 出去、等關機訊號），大部分時間 park；排程去中心化，沒有中央指揮
- 不讓 main 當 worker：`block_on` 可由任意執行緒呼叫；main 返回則整個 process 結束（OS 規則）
- `current_thread` flavor：單執行緒，可用 `spawn_local` + `Rc`
- tokio 無 sysmon（Go 有）：無搶占，長計算自己負責
- 信任 tokio：Rust 刻意把 runtime 留給 crate（嵌入式、kernel、遊戲引擎場景不同，零成本原則）；tokio 用於 Pingora、Deno、Linkerd、AWS SDK；`loom` 窮舉交錯測試、Miri；1.0 長期穩定 + LTS；風險是生態綁定、協作式排程責任在你
- 自己切割執行緒：std 就夠（手寫 thread pool = Rust Book ch.20）；進階：多 runtime、專用 tick 執行緒（遊戲伺服器）、`rayon`、CPU affinity、thread-per-core（`monoio`/`glommio`，不需 Send）、自寫 executor
- 容器坑：K8s CPU limit 不隱藏核心數，tokio 可能開過多 worker；明確設 `worker_threads`

---

## 8. 作業系統層

### process 與 thread
- process = **資源容器**（獨立記憶體空間、fd 表、權限），動態向 kernel 要資源，kernel 在請求時檢查限制（不是預先裝滿的盒子）
- thread = **排程單位**，OS 真正放到 CPU 上的東西；同 process 的執行緒共享記憶體（Arc 能跨執行緒共享的原因）
- main/worker/blocking 在 OS 眼中完全一樣，角色是 tokio 賦予的
- OS 排程器直接排所有 process 的執行緒，預設不分 process；要隔離用 nice、affinity、cgroups、即時排程
- 執行緒數超過核心數是常態；只有**可執行**的執行緒爭搶 CPU，睡著的不算；CPU 密集的可執行數遠超核心數才有害
- 邏輯 CPU（含超執行緒 ≈ 2N，多 20～30% 吞吐）vs 物理核心 N；tokio 用邏輯數

### 限制與預設值
| 限制 | 範圍 | 預設 | 查看 |
|---|---|---|---|
| `ulimit -n` | 每 process 同時開著的 fd | soft 常見 1024（systemd 常拉高） | `ulimit -n` |
| `ulimit -u` | 每使用者執行緒總數 | 依記憶體，數萬～十幾萬 | `ulimit -u` |
| `ulimit -s` | 每執行緒 stack | 8192 KB | `ulimit -s` |
| `threads-max` | 整機執行緒數 | 依記憶體 | `/proc/sys/kernel/threads-max` |
| `pid_max` | 整機號碼池 | 舊 32768、現代 4194304 | `/proc/sys/kernel/pid_max` |
| `file-max` | 整機 fd 數 | 依記憶體 | `/proc/sys/fs/file-max` |
| cgroup `pids.max` | 容器內執行緒數 | 不限（常被設幾千） | K8s `podPidsLimit` |

- Linux 每條執行緒都有 TID，與 PID 共用號碼池；對外 PID = 主執行緒的 TID（TGID）；`ps -eLf` 看全部
- cgroup = 把一群 process 分組限資源（cpu、memory、pids、io）；容器 = cgroup + namespace
- 撞到執行緒限制：`thread::spawn` 回 `Err`（`Resource temporarily unavailable`）；撞到 fd 限制：`accept` 回 `Too many open files`

### user mode ↔ kernel mode
- CPU 兩種特權等級；切換由硬體自動完成，只有兩個觸發：執行 `syscall` 指令（自願）、中斷/例外（非自願）；應用程式無法自己切
- kernel **不是 process**：它是有特權的程式碼，在**呼叫者自己的執行緒**上執行（換制服進限制區，本人去做）；排程器也只是 kernel 的一段程式碼
- 每條執行緒兩個 stack：user stack（8 MB/2 MB）與 kernel stack（16 KB，user 不可存取，安全+可靠）；同一條執行緒在兩種模式用兩塊記憶體
- 虛擬位址空間分上下半：kernel 半（所有 process 共用映射、user 不可存取）、user 半（各自不同）
- 切換單位是「此刻在這顆核心上的那條執行緒」，不是 pid；其他核心、其他執行緒不受影響
- 中斷打斷任何當時在那顆核心上的東西（可能是瀏覽器），受益者可能是你的 worker；處理完 `iret` 回去，被打斷者毫無察覺；除非排程器趁時鐘中斷（每 1～4 ms）換人 → 搶占
- driver 是 kernel 的程式碼（module），不是 process；TCP 堆疊一份程式碼 + 每連線一個 socket struct
- **kernel 從不主動呼叫 user 程式碼**（signal 除外）；app 知道事件的唯一方式是「某條執行緒的 syscall 回傳了」：睡在 `epoll_wait` 被喚醒而回傳，或忙時 61 tick 後呼叫 `epoll_wait(0)` 立刻回傳累積清單 → 全程 pull 模型
- 喚醒兩步：中斷當下標 RUNNABLE（µs）→ 有閒置核心則 IPI 立刻接手（µs），全忙則排隊（ms）；沒有人「戳」睡著的執行緒
- 兩層對稱：app↔tokio 的 `poll`/Pending/Waker/`wake()` ≡ tokio↔kernel 的 `epoll_wait`/SLEEPING/wait queue/RUNNABLE；永遠是問的那一方主動
- 這個模型與 tokio 無關：std 也跨同一條線（`println!`→`write`、`fs::read`→`read`、`thread::spawn`→`clone`、`Mutex`→`futex`）；差別只在 std 睡在阻塞 `read`（一執行緒守一 fd），tokio 睡在 `epoll_wait`（一執行緒守一張清單）
- 任何碰 kernel 資源的動作都要進 kernel；純計算、`Vec::push`（allocator 在 user mode，偶爾 `mmap`）不用
- Socket.IO/WebSocket/HTTP 的協定邏輯在 user mode，只有 TCP 以下的 bytes 收發跨邊界
- 切換約 0.1～1 µs，次數累積有感 → `BufReader`/`BufWriter`、io_uring 減少 syscall；`strace ./bin` 可看每一個 syscall

### fd 與 syscall
- fd = process fd 表的索引，指向 kernel 物件（socket、檔案、pipe、epoll 實例）；每 process 一份，全執行緒共用
- 一條 TCP 連線 = 一個 socket fd；epoll 實例、監聽 socket 各佔一個
- syscall 是動作不佔 fd：建立 fd（`socket/accept/open/epoll_create`）、使用 fd（`read/write/epoll_ctl/epoll_wait`）、無關 fd（`clone/futex/mmap/nanosleep`）
- `ulimit -n` = 同時握著多少 kernel 資源把手（餐廳座位 vs 點餐次數）

### epoll
- 每呼叫一次 `epoll_create` 一個實例（tokio 每 runtime 一個 = I/O driver）；各有興趣清單、就緒清單、wait queue；不是 OS 一份也不是每 pid 一份
- 登記時在 socket 的 wait queue 掛回呼；封包到達 → kernel 沿回呼把 fd 推進該實例的就緒清單 → 喚醒 wait queue 裡的執行緒
- `poll`/`select`：每次傳全名單，kernel O(n) 掃，app 再 O(n) 找 → C10K 瓶頸
- `epoll`：登記一次，kernel 維護就緒清單，`epoll_wait` O(就緒數) → delta
- tokio 用 edge-triggered，狀態變化通知一次
- 平台：Linux epoll、macOS kqueue（就緒式）、Windows IOCP、io_uring（完成式）；`mio` 封裝
- C10K → C10M：解決**大量閒置連線的成本**（執行緒模型每連線數十 KB + 排程；async 每連線幾 KB socket + 幾百 bytes task），不是算力；10M 同時有事時由 CPU 吞吐決定、排隊、延遲上升

### 數量級階梯
| 操作 | 時間 | 3 GHz 週期 |
|---|---|---|
| 一條簡單指令 | ~0.3 ns | 1 |
| L1 / L2 / L3 | 1 / 4 / 15 ns | 3 / 12 / 45 |
| RAM | ~100 ns | 300 |
| tokio 切換 task | 幾十 ns | ~100 |
| 系統呼叫 | 0.5～1 µs | ~2,000 |
| OS 切換執行緒 | 1～5 µs | ~10,000 |
| NVMe 讀取 | 20～100 µs | ~100,000 |
| 同資料中心往返 | ~0.5 ms | ~1,500,000 |
| HDD 尋道 | ~10 ms | ~30,000,000 |
| 台灣↔美西往返 | ~150 ms | ~450,000,000 |

- tokio 計時器精度 1 ms
- 一個 tick（ms）= 幾百萬條指令；這張表解釋了 async 存在的理由

---

## 9. 其他概念

### trait
- 定義一組共同行為，`impl Trait for Type` 顯式實作；可有預設方法、關聯型別（`type Output`）、可為空（marker）
- 只定義行為不定義欄位；可為非自己寫的型別（`i32`）實作自己的 trait；預設靜態分派零成本，`dyn` 才動態分派
- Go interface 隱式/無預設方法/一律動態分派；Rust 顯式
- 已遇過的 trait：`Copy`、`Send`、`Sync`、`Future`、`Fn*`、`Debug`/`Display`、`Drop`、`Iterator`
- `Future` 是 std 的 trait 名（非關鍵字）；`async`/`await`/`trait`/`impl`/`Self` 是關鍵字；大寫開頭通常是 std 的型別或 trait

### macro
- 編譯期產生程式碼，展開在呼叫位置；不是函式呼叫，是語法模式比對；無執行期成本；價值是少寫重複
- 認得：`name!(...)`/`name![...]`/`name!{...}` 等價；`#[attr]` 套下一項目、`#![attr]` 套所在範圍
- 宣告式 `macro_rules!`：`(模式) => {展開}` 分支以 `;` 隔開；`$x:expr` 元變數（`expr/ident/ty/tt/literal`）；`$( ... ),+` 重複（`* + ?`）
- 程序式：derive（`#[derive(Debug)]`）、attribute（`#[tokio::main]`，實作是編譯期執行的 Rust 函式）、function-like（`sqlx::query!`）
- Rust vs C：語法樹（`square!(1+2)` = 9）vs 文字替換（= 5）；hygiene
- 參數可能重複求值（`square!(next())` 呼叫兩次）
- 查文件 → rust-analyzer「Expand macro recursively」→ `cargo expand`；所有 macro 做的事都能手寫（除編譯器內建如 `format_args!`）
- 原則：能用函式/泛型就不用 macro；寫 macro 是進階～巨鱷技能

### channel
- 函式庫提供：`std::sync::mpsc`、`tokio::sync::{mpsc, oneshot, broadcast, watch}`
- `Sender` 可 clone、`Receiver`（mpsc）唯一；**所有 Sender drop 自動關閉**（忘記 `drop(tx)` 會永遠等）；沒有內建 `select`，用 `tokio::select!`
- 有界 channel 提供背壓

### `?` 運算子
- 成功取值、失敗立刻 `return Err(e)`；= Go 的 `if err != nil { return err }`；只能用在回傳 `Result`/`Option` 的函式
- `unwrap()` panic vs `?` 往上傳；`spawn_blocking(...).await??` 剝兩層（`JoinError` + 業務錯誤）；`anyhow::Result` 裝任意錯誤

---

## 10. 誤解修正記錄（一路上被校正過的點）

| 原本的理解 | 修正後 |
|---|---|
| heap 與 stack 1:1、同時結束 | 所有權樹；heap 跟著 owner 結束，owner 可 move；Rc/Arc 多 owner |
| 8 MB / 2 MB 是 heap 限制 | 那是 stack；heap 無固定上限 |
| Rc 可以放多執行緒只是沒 atomic | 編譯期禁止；非原子計數會 use-after-free |
| Rc 的作用與 non-blocking 有關 | Rc 解決所有權結構，同步程式也常用 |
| spawn 只是包成 struct，await 才執行 | spawn 立刻提交；await 是等結果 |
| 沒 await 所以 main 結束時 task 結束 | 是 runtime drop 取消 task |
| join! 由 runtime 處理一群 task | 子 Future 不是 task，當前 task 自己輪流 poll |
| join! 等待是因為在 main thread | 是 join! 語意，與執行緒無關；暫停的是 task |
| main thread 是指揮中樞 | 排程去中心化，main 是發起者/守門人 |
| blocking thread 從 worker 撥出 | 獨立的另一批執行緒；兩層排程（OS 搶占 vs tokio 協作） |
| 開執行緒 = 向系統借算力 | 算力固定；多執行緒增加「等待容量」；是執行緒不是 process |
| spawn_blocking 給可接受高延遲的事 | 給「不會讓出執行緒」的操作，保護其他 task |
| 把 I/O 放 spawn_blocking | 只有**同步** I/O；async I/O 是 worker 的主場 |
| Future trait 有 await 方法 | 只有 `poll`；`.await` 是關鍵字 |
| `main_logic.await` | 要先呼叫 `main_logic().await` |
| .await 展開是死循環、執行緒被 block | Pending 時離開函式；等待集中在 epoll_wait |
| polling 有週期 | 事件驅動，Waker 被呼叫才 poll |
| 有一條 job thread 巡邏 waker | 沒有；硬體中斷 + kernel + 閒置 worker 兼任 driver；Go 才有 sysmon |
| macro 展開放在特殊 block | 展開在呼叫位置 |
| macro 是 call func，應看得出參數 | 語法模式比對，看文件 |
| worker 預設 2N、blocking 1 條 | worker = 邏輯 CPU 數、blocking 0～512 |
| process 是已分配好的資源 | 動態請求，kernel 在請求時檢查限制 |
| process 層級就分離了 CPU 競爭 | OS 直接排所有執行緒；要隔離需 cgroup/affinity |
| epoll 是 OS 一份清單、app 認領 | 每 epoll_create 一個實例，kernel 直接投遞 |
| kernel 是高優先 process | 在呼叫者執行緒上執行的特權程式碼 |
| 兩個 stack 因為兩個主體 | 同一執行緒、兩種模式、兩塊記憶體 |
| 切換以 pid 為單位 | 以「此刻在該核心的執行緒」為單位 |
| kernel 通知 app | app 靠 syscall 回傳得知（pull） |
| ulimit -n = kernel API 數 | = 同時開著的 fd（資源把手）數 |
| 每次 syscall 佔 fd | syscall 是動作，fd 是資源編號 |
