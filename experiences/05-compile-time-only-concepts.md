# Experience 05 — 執行期沒差、概念天差地別：只寫給 rustc 看的東西

> 經驗不是筆記：起點是一個實際撞到的問題，往下挖到能解釋它為止。這一篇撞到的是一個落差：`-> &str` 和 `-> &'static str` 編出來的機械碼一模一樣，能寫的程式卻不一樣。順著「效能沒差，那差在哪」把 Rust 裡同一類的概念排開：它們在執行期不存在（或幾乎不存在），只決定哪些程式碼編得過。
> 當時定位：🐣 入門，q05 結案後。
> 起因：問 `fn to_str(&self) -> &'static str` 的 `'static` 不寫會不會自動變成 `'static`，接著問「那有沒有寫效能上沒差？差別只在簽名表達的意思？」，最後一句是「但這對 Rust 的概念天差地別」。
> 接在哪裡：[note 01 §4](../notes/01-memory-model-async-runtime-os.md)（`Copy` 是標籤）、[note 01 §6](../notes/01-memory-model-async-runtime-os.md)（型別大小、niche）、[note 04 §8](../notes/04-borrow-exclusivity-lifetime-elf.md)（lifetime、`'static`、單態化）、[經驗 04](04-rust-vocabulary-rustc-messages.md)（詞彙）。lifetime 的完整推導在 q05 問題 5 的複盤。
> 來源與驗證狀態：§3–§5 標「實測」的大小是在 rustc 1.97.0、64-bit Linux 上用 `size_of` 量的；§4 的 typestate 範例實際編譯過，錯誤訊息是原文。§3 `Send` 與 `Sync` 一節裡標「實測」的，以及表格裡各型別有沒有 `Send` / `Sync`，都用 `fn assert_send<T: Send>()` 這類函式實際編譯確認過；std 的型別定義是簡化過的示意。其餘是依機制的說明。

---

## 0. 一句話版本

Rust 把「這樣寫對不對」的資訊放進型別和簽名，讓 rustc 在編譯期用掉，執行期不留痕跡。所以遇到新概念先問：**它在執行期存在嗎，還是只寫給 rustc 看？** 前者關心佔多少空間、花多少時間；後者關心它讓哪些程式碼編得過。

## 1. 事件：發生了什麼

```
t0  看到 fn to_str(&self) -> &'static str
t1  問：不寫 'static，不會預設成 'static 嗎？body 裡的 "sword" 明明在 .rodata
t2  得到：rustc 補 lifetime 只看簽名那一行，不看 body
t3  問：那有沒有寫，效能上沒差？
t4  得到：沒差，機械碼一樣。差在簽名對呼叫者說了什麼
t5  「但這對 Rust 的概念天差地別」
```

當時的模型：一個東西如果執行期沒有差別，就等於沒有差別。這在多數語言成立，在 Rust 不成立。

## 2. 分層模型（骨架）

| | 只給 rustc 看 | 執行期存在 |
|---|---|---|
| 住在哪 | 原始碼、編譯器的分析結果 | binary 裡的機械碼與資料 |
| 編譯完之後 | 消失 | 留著 |
| 影響什麼 | 哪些程式碼編得過 | 佔多少記憶體、跑多久 |
| 例 | lifetime、`Copy`、`mut`、`Send` | heap 配置、`checked_add` 的檢查、enum 的 tag |

下面分三類：純標籤（§3）、用型別把東西分開（§4）、編完就消失的抽象（§5）。

## 3. 純標籤：執行期完全不存在

**核心規則**：這些東西本身不產生任何機械碼。它們只回答「這樣寫可不可以」。

### lifetime 與 `'static`

- **概念**：寫在 reference 上的「這張借條能用多久」。rustc 用它檢查借條有沒有活得比被借的東西久。
- **細節**：
  - 只有 reference（和裡面裝著 reference 的型別）有 lifetime。`String`、`i64` 沒有。
  - 它不會讓任何東西活久一點，只做檢查，不合格就拒絕編譯。
  - `'a` 是填空，由每個呼叫點決定。`'static` 是填好的答案：永不過期。
  - `'static` 放在回傳是保證（回傳值和參數無關），放在參數是要求（只收永不過期的）。
  - rustc 補省略的 lifetime 只看簽名，不看 body。

### `Copy`

- **概念**：貼在型別上的標籤，意思是「以值使用之後，原本那份照樣能用」。
- **細節**：
  - 是沒有任何 method 的空 trait。複製動作是 rustc 產生的，不是型別作者寫的。
  - copy 和 move 的機械碼相同（照抄值自己的 byte），差在抄完之後原變數還准不准用。
  - 型別裡只要有一個欄位不是 `Copy`，整個型別就不能是 `Copy`。

### `mut`

- **概念**：`let mut x` 表示這個變數之後可以被重新賦值或被 `&mut` 借。
- **細節**：
  - 它屬於**變數**，不屬於型別。`let x: i64` 和 `let mut x: i64` 是同一個型別。
  - 和 `&mut T` 是兩件事：`let mut` 管的是這個名字能不能改，`&mut T` 是一種型別。`let r = &mut v;` 裡 `r` 自己不是 `mut`，但可以透過它改 `v`。

### `&T` 與 `&mut T`

- **概念**：兩種借條。`&T` 可以同時有很多張，只能讀；`&mut T` 同時只能有一張，可以改。
- **細節**：
  - 執行期兩者都只是一個位址，大小相同（實測 `&i64` 和 `&mut i64` 都是 8 bytes）。
  - 差別全在編譯期的規則：多讀 XOR 單寫。
  - `&T` 是 `Copy`，`&mut T` 不是（不然就會有兩張可寫的借條）。

### `pub`

- **概念**：可見性。沒寫 `pub` 的東西只有同一個 module 裡看得到。
- **細節**：
  - struct 本身和它的欄位各自有可見性。`pub struct P { name: String }` 外面看得到 `P`，但碰不到 `name`。
  - 這是「不變量由誰負責」的工具：欄位不公開，外面就只能透過你給的 method 改它。

### `Send` 與 `Sync`

- **概念**：跨 thread 沒有新規則，還是 copy / move / borrow。`Send` 和 `Sync` 只是多問一句：這個動作跨了 thread 之後還安不安全。兩個都是沒有 method 的空 trait，和 `Copy` 同類。

  | 標籤 | 白話 | 單 thread 時對應的動作 |
  |---|---|---|
  | `Send` | 這個值可以**搬**到另一條 thread | copy、move |
  | `Sync` | 這個值可以被多條 thread **同時借**（`&T`） | borrow |

#### 共用的看 `Sync`，獨佔的看 `Send`

判斷方法：送過去之後，實際上誰碰得到 `T`。

| 送到別條 thread 的東西 | 之後誰碰得到 `T` | 性質 | 條件 |
|---|---|---|---|
| `T`（move） | 只有新 thread | 獨佔 | `T: Send` |
| `T`（`Copy`） | 兩邊各有一份，不同的記憶體 | 各自獨佔 | `T: Send` |
| `&mut T` | 只有新 thread，原 thread 被凍結 | 獨佔 | `T: Send` |
| `&T` | 兩條 thread **指著同一塊** | 共用 | `T: Sync` |

- `&mut T` 看的是 `Send`，因為它是獨佔的借條，效果等於暫時搬過去。而且拿著 `&mut T` 真的能用 `std::mem::take` / `replace` 把裡面的 `T` 換出來帶走。
- `Copy` 不改變規則。複製出來的是兩個獨立的值，不算共用，用不到 `Sync`。
- 唯一「複製之後變成共用」的是借條本身：`&T` 是 `Copy`，複製一張送過去，兩邊指著同一個 `T`。所以 `&T` 的 `Send` 條件被定成 `T: Sync`。這也是 `Sync` 的正式定義：`T` 是 `Sync`，若且唯若 `&T` 是 `Send`。
- `Sync` 和「能不能改」無關。`String` 是 `Sync`，但多條 thread 拿著 `&String` 誰都不能改。

#### 兩張獨立的許可

不是「兩個都有才能用在 thread」。各管各的動作。

| `Send` | `Sync` | 能做什麼 | 例 |
|---|---|---|---|
| ✅ | ✅ | 能搬，也能共用 | `i64`、`String`、`Vec<T>`、`Arc<T>`、`Mutex<T>` |
| ✅ | ❌ | 能搬給一條 thread 獨佔，不能共用 | `Cell<T>`、`RefCell<T>` |
| ❌ | ✅ | 只能送 `&T` 過去讓人看（少見） | `MutexGuard` |
| ❌ | ❌ | 完全不能離開原本的 thread | `Rc<T>` |

`RefCell` 整個 move 進一條 thread 是可以的（實測），因為搬走之後原 thread 碰不到它。`Rc` 連搬都不行，因為原 thread 可能還留著別的 `Rc::clone`，兩邊指著同一個計數。

#### 標籤和工具是兩回事

`Send` / `Sync` 是標籤，執行期不存在。`Rc` / `Arc` / `Cell` / `RefCell` / `Mutex` 是工具，執行期真的有計數、有鎖。

工具存在的理由：Rust 預設有兩件事做不到，每件事各有便宜的單 thread 版和較貴的多 thread 版。

| 要鬆開的規則 | 單 thread | 多 thread |
|---|---|---|
| 一個值只有一個擁有者 → 要多個擁有者 | `Rc` | `Arc` |
| 有 `&` 就不能改 → 只有 `&` 也要改，小的值 | `Cell` | `Atomic*` |
| 同上，大的值 | `RefCell` | `Mutex` / `RwLock` |
| 兩個都要 | `Rc<RefCell<T>>` | `Arc<Mutex<T>>` |

- 標籤的工作是保證你不會把左邊的拿去右邊用。
- `Cell` / `RefCell` 是**單 thread** 的工具，不是給 thread 用的。
- `Arc` 和 `Mutex` 各管一件事：`Arc` 管共同**擁有**，裡面 atomic 的只有引用計數，`T` 的內容沒有保護，透過 `Arc` 只拿得到 `&T`。`Mutex` 管輪流**改**。
- `RefCell` 沒有取消「多讀 XOR 單寫」，只是把檢查從編譯期搬到執行期，違規是 panic（實測訊息 `RefCell already borrowed`）。`Mutex` 也是同一條規則搬到執行期，違規是排隊等。
- 名字：`Rc` = Reference Counted，`Arc` = Atomically Reference Counted，`Mutex` = mutual exclusion（互斥，和 `mut` 的 mutable 無關），`RwLock` = Read-Write Lock。

#### `UnsafeCell`：`&` 能不能改的開關（和 thread 無關）

先把這個零件單獨講清楚。它是一般的語言機制，單 thread 的程式照樣用得到。

rustc 看到 `&T`，會認定這張借條還在的期間，它指的資料不會變，而且真的拿這個假設來產生機械碼：

```rust
fn f(x: &u32, g: impl Fn()) -> u32 {
    let a = *x;
    g();
    let b = *x;      // rustc 可以推論 b 一定等於 a，只讀一次記憶體
    a + b
}
```

所以語言規定：透過 `&` 修改資料是未定義行為。**唯一的例外**是資料包在 `UnsafeCell<T>` 裡。它是寫進編譯器的特殊型別，作用只有一個：告訴 rustc「這一格可能在 `&` 後面被改，不要對它做假設」。

```rust
use std::cell::UnsafeCell;

let c = UnsafeCell::new(5);     // 沒有 let mut
let r = &c;                     // 一張普通的 & 借條
unsafe { *r.get() = 10; }       // 透過 & 改了裡面的值（實測印出 10）
```

- 它本身不做任何事：沒有計數、沒有鎖，`UnsafeCell<u32>` 和 `u32` 一樣是 4 bytes（實測）。
- `get()` 交出的是裸指標，要用就得寫 `unsafe`。有沒有別人同時在讀寫，由寫的人自己保證。
- 平常不會直接碰它。std 把它包成安全的型別，各自加一種保護：

  | 型別 | 裡面 | 加了什麼保護 |
  |---|---|---|
  | `Cell<T>` | `UnsafeCell<T>` | 不交出借條，只能整個拿出來或換掉 |
  | `RefCell<T>` | `UnsafeCell<T>` 加計數 | 執行期檢查借用規則，違規 panic |
  | `Mutex<T>` | `UnsafeCell<T>` 加鎖 | 要先搶到鎖 |
  | `Atomic*` | `UnsafeCell<整數>` | 用 CPU 的 atomic 指令讀寫 |

> `UnsafeCell` 只決定一件事：**這個型別的 `&`，還是不是「只能讀」**。它不是 thread 的機制，也不直接決定 `Send` 或 `Sync`。

#### `Sync` 怎麼用到這個事實

資料競爭 = 兩條 thread 同時碰同一塊記憶體，至少一條在寫，而且沒有同步。

一般型別自動是 `Sync`，因為 `&` 只能讀，多條 thread 拿著 `&T` 沒有人能寫。`Sync` 在判斷時需要知道「這個型別的 `&` 是不是只讀」，這時才去看 `UnsafeCell` 在不在。它是被拿來當依據的事實，中間還隔了一步：

```
有 UnsafeCell
   → & 不再保證只讀
      → 多條 thread 同時拿 & 就可能同時寫
         → 所以預設不給 Sync
            → 除非包它的型別另外加了跨 thread 的保護（鎖、atomic），作者再把 Sync 貼回去
```

| 型別 | 有 `UnsafeCell` | `&` 能改 | 有跨 thread 的保護嗎 | `Sync` |
|---|---|---|---|---|
| `String`、自己寫的 `Player` | 沒有 | 不能 | 不需要 | ✅ |
| `Cell`、`RefCell` | 有 | 能 | 沒有 | ❌ |
| `Mutex`、`Atomic*` | 有 | 能 | 有 | ✅ |

`UnsafeCell` 回答的是第三欄。`Sync` 是第五欄，還要看第四欄。所以「追得到 `UnsafeCell`」不等於「不是 `Sync`」。

- **`Send` 不受 `UnsafeCell` 影響。**「透過 `&` 能寫」只在共用時才危險，整個搬給另一條 thread 獨佔沒有問題。所以 `Cell` / `RefCell` 有 `Send`、沒有 `Sync`。
- 讓型別連 `Send` 都沒有的是另一個零件：裸指標。rustc 完全不知道它指向誰、有沒有別人也指著。

| 零件 | `Send` | `Sync` |
|---|---|---|
| `UnsafeCell<T>` | 跟著 `T` | 預設 ❌ |
| 裸指標 `*const T` / `*mut T` | 預設 ❌ | 預設 ❌ |

`Rc` 和 `Arc` 的定義（簡化）：

```rust
pub struct Rc<T>   { ptr: 裸指標 → RcInner<T> }
struct RcInner<T>  { strong: Cell<usize>, weak: Cell<usize>, value: T }

pub struct Arc<T>  { ptr: 裸指標 → ArcInner<T> }
struct ArcInner<T> { strong: AtomicUsize, weak: AtomicUsize, data: T }
```

兩者結構幾乎一樣，差在計數是 `Cell<usize>` 還是 `AtomicUsize`，以及作者有沒有手寫 `unsafe impl` 把標籤貼回去：

| 型別 | 作者做了什麼 | 憑什麼 |
|---|---|---|
| `Cell`、`RefCell`、`Rc` | 沒貼 | 寫入和計數都是普通的讀、改、寫三步，會被插隊 |
| `Arc<T>` | `unsafe impl<T: Send + Sync> Send / Sync` | 計數用 CPU 的 atomic 指令 |
| `Mutex<T>` | `unsafe impl<T: Send> Send / Sync` | 碰資料前一定先經過鎖 |
| `Atomic*` | `unsafe impl Sync` | 每個操作都是 atomic 指令 |

```
你的程式碼   struct Player { … }               全自動：欄位都合格就合格
──────────────────────────────────────────────────────────
std         Rc / Arc / Cell / RefCell / Mutex   用下面的零件組出來
            unsafe impl Send / Sync             保護夠的那幾個由作者擔保
──────────────────────────────────────────────────────────
語言        UnsafeCell                          「& 可以改」的開關，所以預設不給 Sync
            裸指標                              rustc 不知道它指向誰，所以預設都不給
──────────────────────────────────────────────────────────
CPU         普通的 讀-改-寫 三步會被插隊；atomic 指令不會
```

寫程式時不用想 `UnsafeCell`，它是 std 作者用的零件。日常只要記：

| struct 的欄位裡有 | 能搬到別條 thread | 能被多條 thread 共用 |
|---|---|---|
| `Rc` | ❌ | ❌ |
| `Cell`、`RefCell` | ✅ | ❌ |
| 其他（包含 `Arc`、`Mutex`、`Atomic*`） | ✅ | ✅ |

#### 怎麼判斷一個型別有沒有標籤

「欄位都是 `Send`，它才是 `Send`」不是循環定義，是有底的遞迴：

```
這個型別的作者有手動標嗎？
├─ 有   → 照他標的（Arc、Mutex、Rc、Vec…）
└─ 沒有 → 對每個欄位重複這個問題，
          一路拆到基本型別（語言規定都有）、UnsafeCell（預設沒有 Sync）、
          裸指標（預設都沒有）為止
```

- 自己寫的 struct 永遠走「沒有」那條。結論：裡面有沒有藏 `Rc`、`Cell`、`RefCell`、裸指標。藏了一個，不管包幾層，整個型別都過不了。
- rustc **只看欄位，不看 method**。有 `&mut self` 的 method 不影響 `Sync`：`Sync` 只承諾多條 thread 同時拿 `&T`，而透過 `&` 呼叫不了 `&mut self` 的 method。那種 method 由借用規則把關（有 `&` 的時候借不出 `&mut`，跨 thread 也一樣，實測是 E0502）。
- 想讓多條 thread 都能改，不是把 method 改成 `&self`，是把欄位的型別換成 `Mutex` 或 `Atomic*`，或在外面包 `Arc<Mutex<…>>`。包整個 struct 時，裡面的 method 維持 `&mut self` 就好。
- 直接問 rustc：

  ```rust
  fn assert_send<T: Send>() {}
  fn assert_sync<T: Sync>() {}

  assert_send::<Player>();   // 編得過就是有
  assert_sync::<Player>();
  ```

  編不過時，訊息裡的 `required for … to implement …` 從下往上讀，就是 rustc 拆的過程。

#### rustc 在哪裡檢查

沒有特殊處理，就是 std 的一行普通簽名：

```rust
pub fn spawn<F>(f: F) -> JoinHandle<…>
where
    F: FnOnce() + Send + 'static
```

closure 是一個裝著被抓變數的 struct，抓了什麼就由什麼組成。兩個條件是獨立的檢查：

| 檢查 | 問的是 | 不過時的錯誤（實測） |
|---|---|---|
| `Send` | 型別跨 thread 安不安全 | E0277 `` `Rc<i32>` cannot be sent between threads safely `` |
| `'static` | 它依賴的東西會不會比 thread 早消失 | E0373 `closure may outlive the current function, but it borrows …` |

closure 不寫 `move` 預設是借，寫 `move` 是搬（或複製）。`spawn` 因為 `'static` 幾乎都得寫 `move`。

| 想做的事 | 寫法 | 檢查什麼 |
|---|---|---|
| 把值交給 thread | `thread::spawn(move \|\| …)` | `T: Send` |
| 借給 thread，thread 很快結束 | `thread::scope` 裡 `scope.spawn(\|\| …)` | `T: Sync`，壽命由 `scope` 保證 |
| 多條 thread 長期共用 | `Arc::clone` 之後 `move` 進去 | `T: Send + Sync` |

`move` 是整個 closure 一起生效的。想「這個搬、那個借」，先做一張借條 `let r = &b;`，再把借條 `move` 進去。

#### 什麼時候才需要 `Arc<Mutex<T>>`

先問兩個問題：

| | 只有一條 thread 碰它 | 多條 thread 都要碰它 |
|---|---|---|
| 只讀 | 直接 move 進去 | `Arc<T>`，不用 `Mutex` |
| 要改 | 直接 move 進去，`let mut` | `Arc<Mutex<T>>` |

只有右下角需要。右下角也還有別的做法：讀多寫少用 `RwLock`，單一數字用 `Atomic*`，或者乾脆不共用（channel：資料只給一條 thread 擁有，其他人送訊息過去）。用 `thread::scope` 時 `Arc` 可以省，直接借一個區域的 `Mutex`（實測）。

### `unsafe`

- **概念**：標出「這一段的安全性由寫的人保證，不是 rustc」。
- **細節**：
  - 它不會關掉 borrow checker，也不會讓程式跑得不一樣。
  - 它只多開放五件事：解參考 raw pointer、呼叫 `unsafe` 函式、存取可變的 `static`、實作 `unsafe` trait、讀 union 的欄位。
  - 作用是把需要人工審查的範圍圈出來。

### `#[must_use]`

- **概念**：貼在型別或函式上，意思是「這個回傳值不該被直接丟掉」。
- **細節**：
  - 丟掉時是 warning，不是 error。
  - `Result` 就有貼，所以呼叫一個回 `Result` 的函式卻不處理會被提醒。
  - 真的要丟，寫 `let _ = …;` 表示是故意的。

## 4. 用型別把東西分開：執行期長得一樣（或幾乎一樣）

**核心規則**：記憶體裡是同樣的 bytes，但型別不同，rustc 就不讓你混用。

### enum 取代字串或數字

- **概念**：把「只有這幾種」寫進型別。
- **細節**：
  - 執行期是存在的：一個小整數 tag（實測四個 variant 的 `Op` 是 1 byte），比字串比對便宜。
  - 編譯期多出來的是**封閉**：不可能出現第五種，`match` 漏了一種就編不過。
  - 用字串時，「只有四種」這件事只在你腦袋裡；用 enum 時在型別裡。

### newtype

- **概念**：只包一個欄位的 struct，`struct Meters(f64);`。目的是做出一個新的、不能和原型別混用的型別。
- **細節**：
  - 大小和裡面那個型別相同（實測 `Meters` 和 `f64` 都是 8 bytes）。
  - 不會繼承裡面型別的 method 和運算子。要用得自己實作，或用 `.0` 拿出裡面的值。
  - `Meters` 和 `Seconds` 都包 `f64`，但互相不能傳、不能相加。
  - 另一個用途是繞過 orphan rule：把別人的型別包一層，就變成「自己的型別」，可以幫它實作別人的 trait。

### `Option<T>`

- **概念**：把「可能沒有」寫進型別。要用裡面的值一定得先拆。
- **細節**：
  - 成本看 `T`。`T` 有不可能出現的值（reference 不會是 0）時，`None` 借那個值來表示，大小不變（實測 `&u8` 和 `Option<&u8>` 都是 8）。
  - `T` 的每種 bit 組合都合法時，要多一格 tag（實測 `u32` 是 4，`Option<u32>` 是 8）。
  - 和 null 的差別不在執行期，在「忘記檢查」會不會編過。

### `Result<T, E>`

- **概念**：把「可能失敗，以及失敗的原因」寫進回傳型別。
- **細節**：
  - 執行期是一個帶 tag 的值加一次分支，不像例外需要沿著 stack 找處理的人。
  - 會不會失敗在簽名上就看得到，呼叫者不能假裝不知道。
  - `?` 只是把「`Err` 就往上 return」寫短，沒有隱藏的控制流。

### 大小為 0 的型別

- **概念**：不佔任何空間的型別，只在型別層有意義。`()`、`struct Marker;`、`PhantomData<T>`。
- **細節**：
  - 實測三者 `size_of` 都是 0。沒有捕捉任何變數的 closure 也是 0。
  - `Vec<()>` 不會配置 heap，因為元素不佔空間。
  - `PhantomData<T>` 是告訴 rustc「把我當成持有一個 `T`」，用來帶型別參數卻不真的存它。

### typestate

- **概念**：把物件的狀態寫成型別參數，讓「在錯誤的狀態呼叫某個 method」變成編譯錯誤。
- **細節**：
  - 狀態轉換的 method 吃掉 `self`、回傳另一個型別的值，舊狀態的變數就不能再用。
  - 狀態型別是大小為 0 的標記，所以不佔空間（實測 `Conn<Open>` 和裡面的 `i32` 一樣是 4 bytes）。

  ```rust
  use std::marker::PhantomData;

  struct Open;
  struct Closed;
  struct Conn<S> { fd: i32, _s: PhantomData<S> }

  impl Conn<Closed> {
      fn open(self) -> Conn<Open> { Conn { fd: self.fd, _s: PhantomData } }
  }
  impl Conn<Open> {
      fn send(&self) -> i32 { self.fd }
  }
  ```

  對還沒 `open` 的連線呼叫 `send`：

  ```
  error[E0599]: no method named `send` found for struct `Conn<Closed>` in the current scope
  ```

  不是執行期檢查「狀態對不對」，是那個 method 在這個型別上根本不存在。

## 5. 編完就消失的抽象

**核心規則**：寫的時候是高階的寫法，rustc 把它展開成和手寫一樣的機械碼。成本付在編譯時間和 binary 大小，不付在執行期。

### generics

- **概念**：函式或型別留一個型別的洞。
- **細節**：
  - rustc 幫每個實際用到的型別各產生一份程式碼（單態化）。`max::<i64>` 和 `max::<f64>` 是兩個獨立的函式。
  - 每一份都知道確切的型別，所以可以內聯、可以最佳化，和手寫的一樣快。
  - 代價是編譯變慢、binary 變大。

### iterator

- **概念**：一個會一次吐出一個值的東西。`.filter(…).map(…)` 是在它外面再包一層。
- **細節**：
  - 是惰性的。`.filter()`、`.map()` 只是組裝，什麼都還沒跑。要有人來拿（`for`、`.collect()`、`.sum()`）才開始動。
  - 每一層是一個小 struct，rustc 把它們內聯之後通常變成一個迴圈。
  - 不會產生中間的 `Vec`，除非你自己 `.collect()`。

### closure

- **概念**：可以抓住周圍變數的匿名函式，`|x| x + n`。
- **細節**：
  - rustc 把它編成一個匿名 struct（欄位是抓到的變數）加一個函式。
  - 大小就是抓到的東西的大小（實測：借一個 `i64` 是 8 bytes、`move` 一個 `String` 是 24、什麼都沒抓是 0）。
  - 不會配置 heap，除非你把它放進 `Box<dyn Fn…>`。
  - 抓的方式（借、可變借、搬進來）由 body 怎麼用決定，`move` 關鍵字強制搬進來。

### 靜態分派與 `dyn Trait`

- **概念**：呼叫 trait 的 method 時，「到底呼叫哪個函式」在編譯期決定（靜態）還是執行期決定（動態）。
- **細節**：

  | | 靜態（generics、`impl Trait`） | 動態（`dyn Trait`） |
  |---|---|---|
  | 何時決定呼叫誰 | 編譯期 | 執行期，查表 |
  | 指標大小 | 一般指標，8 bytes | 兩個指標：資料加函式表，實測 16 bytes |
  | 能內聯嗎 | 能 | 不能 |
  | 程式碼份數 | 每個型別一份 | 只有一份 |
  | 能把不同型別放進同一個 `Vec` 嗎 | 不能 | 能 |

  - `dyn` 是這一類裡唯一「主動選擇付執行期成本」的。換到的是彈性。

## 6. 解法對照：遇到新概念怎麼判斷

| 做法 | 結果 | 採用 |
|---|---|---|
| 先問「它在執行期存在嗎」 | 只給 rustc 看 → 研究它讓哪些程式碼編得過。執行期存在 → 研究空間和時間 | ✅ |
| 用 `size_of` 量、拿掉它看 binary 會不會變 | 直接驗證 | ✅ |
| 用「效能有沒有差」判斷重不重要 | 會把 lifetime、`Copy`、`Send` 全都當成不重要 | ❌ |
| 從 body 推測簽名的意思 | rustc 檢查呼叫點時只看簽名 | ❌ |

## 7. 陷阱清單

1. ❌ `'static` 就是寫在 `.rodata` 的東西 → ✅ `'static` 是「不依賴任何會過期的東西」；`String` 這種自己擁有資料的型別也符合 → 為什麼：`.rodata` 的字面值只是最常見的例子，不是定義。
2. ❌ body 回傳的是字面值，所以不寫 `'static` 也會自動是 `'static` → ✅ rustc 補省略的 lifetime 只看簽名；沒寫就照省略規則綁到參數 → 為什麼：簽名是合約，不能因為 body 改了就自動變。
3. ❌ 執行期沒差，所以寫不寫都一樣 → ✅ 差在呼叫者能寫什麼：`-> &'static str` 讓呼叫者可以在回傳值還在用時把 `self` 交出去 → 為什麼：rustc 拿呼叫點對簽名檢查。
4. ❌ `'static` 是預設的、不確定的狀態，`'a` 是需要時才配置 → ✅ `'static` 是最確定的一個值（永不過期），`'a` 才是待填的空；兩者都不配置任何東西 → 為什麼：lifetime 在執行期不存在。
5. ❌ 寫了 lifetime，被借的東西就會活久一點 → ✅ lifetime 只做檢查，不合格的結局是編不過 → 為什麼：東西何時釋放只看擁有者。
6. ❌ `Copy` 是一個 method → ✅ 是空的標籤 → 為什麼：複製動作是 rustc 產生的。
7. ❌ 型別要同時有 `Send` 和 `Sync` 才能用在 thread → ✅ 兩張獨立的許可：`Send` 管搬，`Sync` 管共用 → 為什麼：`RefCell` 只有 `Send`，整個 move 進一條 thread 是可以的。
8. ❌ `Sync` 的意思是可以在別條 thread 裡 mutable → ✅ `Sync` 只說 `&T` 可以被多條 thread 同時拿，和能不能改無關 → 為什麼：`String` 是 `Sync`，但透過 `&String` 誰都不能改。
9. ❌ `Arc` 表示這個型別滿足原子性 → ✅ atomic 的只有引用計數，`T` 的內容沒有保護 → 為什麼：`Arc` 管共同擁有，要改還得加 `Mutex`。
10. ❌ struct 有能改欄位的 method，就不能是 `Sync` → ✅ `&mut self` 的 method 不影響；只有欄位裡有「能透過 `&` 改」的型別才不是 `Sync` → 為什麼：rustc 只看欄位不看 method，而透過 `&` 呼叫不了 `&mut self` 的 method。
11. ❌ 多條 thread 要改，就把資料包成 `Cell` → ✅ 包成 `Mutex` 或 `Atomic*` → 為什麼：`Cell` 能透過 `&` 改但沒有保護，所以沒有 `Sync`。
12. ❌ `Cell` / `RefCell` 是給 thread 用的 → ✅ 它們是單 thread 的工具，用在「好幾個地方共用又要改」 → 為什麼：講 thread 時提到它們，是當成不能共用的反例。
13. ❌ `&mut T` 是借條，跨 thread 應該看 `Sync` → ✅ 看 `T: Send` → 為什麼：共用的看 `Sync`，獨佔的看 `Send`；`&mut` 是獨佔。
14. ❌「欄位都 `Send` 才 `Send`」是循環定義 → ✅ 是有底的遞迴，底是基本型別、`UnsafeCell`、裸指標，和 std 作者手寫的 `unsafe impl` → 為什麼：拆到不能再拆的地方，答案是直接規定好的。
15. ❌ `Mutex` 的 mut 是 mutable → ✅ mutual exclusion，互斥 → 為什麼：名字是巧合。但功能接得上：`&mut` 在編譯期保證獨佔，`Mutex` 在執行期保證獨佔。
16. ❌ `UnsafeCell` 是處理 thread 的機制 → ✅ 它只是「`&` 能不能改」的開關，單 thread 的 `Cell` / `RefCell` 照樣用它 → 為什麼：`Sync` 判斷時參考它，但它本身和 thread 無關。
17. ❌ struct 裡追得到 `UnsafeCell`，就必然不是 `Send` 也不是 `Sync` → ✅ `Send` 不受它影響；`Sync` 是預設沒有，`Mutex`、`Atomic*` 這種加了保護的，作者有貼回去 → 為什麼：「`&` 能改」只在共用時危險，而且危不危險還要看有沒有鎖或 atomic。
