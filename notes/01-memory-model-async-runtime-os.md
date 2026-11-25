# Rust 學習筆記 01：從記憶體模型到 async runtime 與作業系統

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

> 完整模型在第 4 節：同一張傳參表，從「照抄值自己的 byte + 型別有沒有 `Copy`」重新推一次，並補上 clone 與 borrow。

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

> 「必須放 heap 的根本原因」那一條的方向，第 4 節修正過：大小固定只是放 stack 的資格；值會在 heap，是因為 `String` 這類型別的程式碼呼叫了 allocator，不是 rustc 看大小替你決定。
>
> stack 與 heap 以外的區域（`.rodata`、`.data`、`.bss`）、各區域的存取成本，以及 kernel 什麼時候進場，在第 5 節。
>
> 型別佔多少 byte、enum 的佈局、函式的機器碼與 stack frame 的邊界，在第 6 節。

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

## 4. Copy / move / clone / borrow

> 回頭補的一節。第 2、3 節是最初的概覽；這裡把同一件事從「機器只照抄值自己的 byte」這一條規則重新推一次，並修正那兩節留下的幾個方向（見本節的陷阱清單）。標 **補強** 的 blockquote 是整理進 repo 時加註的，不在原本這條線的對話裡。

### 一句話版本

以值使用任何東西，機器都只照抄「值自己的 byte」。抄完之後原變數還能不能用，由型別有沒有實作 `Copy` 決定。borrow 是同一條規則套在「位址」這種值上。`clone` 是唯一會去複製「指向的內容」的動作。

### 分層

| 層 | 問的問題 | 答案 |
|---|---|---|
| 機器 | 傳遞時實際做什麼 | 一律照抄值自己的 byte |
| rustc | 抄完原變數還能用嗎 | 型別是 `Copy`：能。不是：失效（move） |
| rustc | 傳的是位址時 | 擁有權不變，另外檢查 lifetime 與讀寫規則（borrow） |
| std | `clone()` 做什麼 | 一個 method，行為由型別作者決定 |

Rust 沒有 call by reference 這種獨立機制。參考本身就是一個值，傳參考就是傳值。

### 值自己的 byte 與指向的內容

**核心規則**：一個值是一塊固定大小的 byte。這一塊裡如果有指標，指向的是另外一塊。move 和 `Copy` 只抄前者，`clone` 才會碰後者。

```
原本
a [ptr | 2 | 2] ──► heap P: h i

let b = a;            move
a （失效）
b [ptr | 2 | 2] ──► heap P: h i      同一塊，沒有新配置

let b = a.clone();    clone
a [ptr | 2 | 2] ──► heap P: h i
b [ptr | 2 | 2] ──► heap Q: h i      新配置一塊，內容複製過去

let b = &a;           borrow
a [ptr | 2 | 2] ──► heap P: h i
b [ptr] ──► 指向 a
```

| 動作 | 寫法 | 抄了什麼 | 新的 heap 配置 | 擁有者 | 原變數之後 |
|---|---|---|---|---|---|
| 以值傳，型別是 `Copy` | `f(x)` | 值自己的 byte | 0 | 各自獨立 | 能用 |
| move | `f(s)` | 值自己的 byte（`String` 是 24） | 0 | 1，換人 | 永久失效，除非重新賦值 |
| borrow | `f(&s)` | 8 byte 位址（`&str` 是 16） | 0 | 1，沒變 | 借用結束後能用 |
| clone | `f(s.clone())` | 值自己的 byte + 內容 | 依型別，`String` 是 1 | 2，各擁有一塊 | 能用 |

**關鍵細節**

- move 不是「沒有複製」，是「複製了，而且 rustc 不准再碰原本那份」。「標記失效」純編譯期，執行檔裡沒有對應的指令。
- move 之後釋放只發生一次，在新擁有者離開作用域時。原變數不會被釋放，也不會自己恢復。
- move 抄的 byte 裡有沒有位址要看型別：`String` 有、`Box<T>` 整個就是、沒加 `Copy` 的 `struct P { x: i32 }` 沒有。
- borrow 期間原變數受限是暫時的：被 `&` 借出時能讀不能改，被 `&mut` 借出時完全不能碰，借用結束全部恢復。
- `clone` 不一定貴：`String`、`Vec` 是深複製；`Copy` 型別的 clone 就是照抄；`Rc`、`Arc` 的 clone 只把計數加一。

> **補強**：「標記失效純編譯期」有一個例外。變數只在某些分支被 move（`if cond { take(s); }`）時，rustc 在編譯期無法確定作用域結束時 `s` 還在不在，會在 stack 上多放一個隱藏的布林（drop flag），執行期照它決定要不要 drop。「之後能不能再用 `s`」仍然是編譯期判定；執行期多出來的只有「要不要釋放」這一個檢查。

### `Copy` 是什麼

**核心規則**：`Copy` 是一個沒有任何 method 的 trait，是貼在**型別**上的標籤，只回答「照抄之後原本那份還安不安全」。預設是 move，`Copy` 要作者主動宣告。

```rust
pub trait Copy: Clone { }      // std 裡的定義，大括號是空的

#[derive(Clone, Copy)]         // Copy 要求先有 Clone
struct Point { x: i32, y: i32 }
```

```
rustc 看到 let b = a;
  → 查 a 的型別 T 有沒有 impl Copy
      有   → a 維持可用
      沒有 → 在 MIR 上把 a 標記為「已移出」，之後再用就報 E0382
```

能不能 `Copy` 的判斷標準是「**有沒有擁有需要釋放的東西**」，不是「有沒有用到 heap」：

| 型別 | 跟 heap 有關嗎 | 擁有要釋放的東西嗎 | `Copy` |
|---|---|---|---|
| `i32`、`bool`、`char`、`f64` | 無 | 無 | 是 |
| `&T`、`&str`（即使指向 heap） | 可能指向 heap | 沒有，只是借 | 是 |
| `&mut T` | | 沒有，但必須獨佔 | 不是 |
| `String`、`Vec`、`Box` | 內容在 heap | 有 | 不是 |
| `File` | 無 | 有，要關檔 | 不是 |
| `[T; N]`、`(A, B)` | | 看內容 | 內容全是 `Copy` 才是 |
| 自訂 struct | | | 預設不是；欄位全是 `Copy` 且有 derive 才是 |

**關鍵細節**

- 為什麼不是 `Copy` 就得 move：照抄 `String` 的 24 byte 會得到兩個指向同一塊 heap 的擁有者，離開作用域時釋放兩次（double free），或一方搬家後另一方指向已釋放的記憶體。
- 加 `Copy` 的兩個條件：所有欄位都是 `Copy`，而且型別沒有實作 `Drop`。不符合會報 E0204。
- 自訂 struct 即使沒有需要釋放的欄位，也可以不加 `Copy`，那它就是 move。
- 自己寫一個叫 `copy` 的 method 沒有任何效果，rustc 只認 trait。
- 預設 move 的理由：忘了宣告頂多不方便；而且 `Copy` 是對使用者的承諾，日後加一個 `String` 欄位就得收回。

> **補強**：兩個條件對應兩個不同的錯誤碼。欄位不是 `Copy` 報 E0204；型別實作了 `Drop` 報的是 E0184（`rustc --explain E0184`）。

| | `Copy` | `Clone` |
|---|---|---|
| 有 method 嗎 | 沒有 | `fn clone(&self) -> Self` |
| 複製由誰執行 | rustc 產生的照抄 byte | 型別作者寫的程式碼 |
| 怎麼觸發 | 自動，以值使用時 | 明確寫 `.clone()` |
| 能做什麼 | 只能原樣照抄 | 任何事，例如配置新的 heap |

### 位置與 `Copy` 是兩個獨立的問題

**核心規則**

| 問題 | 由什麼決定 |
|---|---|
| 這個值現在在哪 | 你怎麼放它 |
| 這個值能不能 `Copy` | 它的型別 |

| 怎麼建立 | 值本體在哪 | 對大小的要求 |
|---|---|---|
| 字面值、`static` | 執行檔唯讀區 | 編譯期就要知道內容 |
| `let` 區域變數 | stack | 必須大小固定 |
| `Box::new(..)`、`v.push(..)` | heap | 不限 |

> 第一列在第 5 節細分過：不可變的 `static` 才在唯讀區；`Atomic`、`Mutex` 這類可以修改的 `static` 在 `.data` 或 `.bss`；`const` 則沒有自己的位置。

**關鍵細節**

- rustc 從不自動配置 heap。會配置的只有 `String`、`Vec`、`Box` 這類型別的程式碼去呼叫 allocator。
- 大小固定是放 stack 的必要條件，不是原因。
- `str`、`[T]` 這類不定大小型別不能當區域變數（`let ss = *s;` 會錯），只能躲在帶長度的指標後面用。
- `&X` 指向 X 所在的位置：`&Box<T>` 指向 `Box` 變數本身（通常在 stack），`&*b` 才指到 heap。
- 同一個 `&str` 型別可以指向執行檔、heap 或 stack，型別不告訴你 byte 在哪。
- `Copy` 的來源和目的地在哪都可以；複製出來的那一份在**目的地**：

| 寫法 | 目的地 | 那一份在哪 |
|---|---|---|
| `let b = a;` | 區域變數 | stack |
| `f(a)` | `f` 的參數 | stack，`f` 的 frame |
| `v.push(a)`、`v[0] = a;` | `Vec` 的格子 | heap |
| `*boxed = a;`、`Box::new(a)` | `Box` 指向的那塊 | heap |
| `let b = v[0];` | 區域變數 | stack |

struct 的佈局：所有欄位自己的 byte 都排在 struct 本體那一塊裡；`String` 欄位在本體裡佔 24 byte，內容永遠在另一塊，不管本體在哪。

```
let a = Player { .. };
stack                               heap
a [hp | name: ptr,len,cap] ───────► F e r r i s

Vec<Player> 裝兩個
stack
v [ptr | len 2 | cap 2]
   │
   ▼ heap 區塊 A：Vec 的緩衝區，連續
   [hp | ptr,len,cap][hp | ptr,len,cap]
          │                 │
          ▼ heap 區塊 B     ▼ heap 區塊 C
          F e r r i s       C o r r o
```

- `size_of::<Player>()` 固定是 32，跟名字多長無關。
- 不能做成一整塊連續的原因：`Vec` 靠「每個元素一樣大」算位置，而且名字要能變長。
- 釋放由內而外：每個 `name` 先釋放自己那塊，最後 `Vec` 釋放緩衝區。

### stack frame 與複製次數

**核心規則**：stack frame 的大小是 rustc 在編譯期算好的（所有區域變數的總和），進入函式時一次保留。函式內所有的 `let`、複製、move 都只是往這塊已保留的空間寫值。

> 「所有區域變數的總和」在第 6 節修正過：那是主要來源，另外還有暫時值、padding、返回位址；frame 的邊界、和 page 的關係也在那裡。

**關鍵細節**

- stack 用量由「有幾個變數、各多大」決定，不由「複製了幾次」決定。迴圈裡的 `let b = a;` 跑一百萬次，都是寫進同一格。
- copy 與 move 在 stack 用量上沒有差別。
- 會 stack overflow 的只有兩種：呼叫層數太深、單一區域變數太大。
- `Copy` 不保證便宜：`[u8; 4096]` 可以是 `Copy`，但每次以值傳遞都悄悄抄 4 KB，耗的是 CPU 時間。

### 怎麼選

| 函式要做的事 | 參數型別 |
|---|---|
| 只是讀 | `&T` |
| 要修改，呼叫端之後還要用 | `&mut T` |
| 要把值存起來、帶走或消耗掉 | `T`（交出擁有權） |
| 值很小又單純（數字、座標） | `T`，靠 `Copy` |

- 預設先借。函式真的需要擁有才收 `T`。確實需要第二份才 `.clone()`。
- 自己的型別：小而純的資料（座標、顏色、ID、狀態列舉）加 `Copy`，其他不加。
- 不要問「move 好還是 copy 好」，那是型別決定的。要問的是「這個函式需不需要擁有它」。

### 透過借用能做什麼：不能 move

**核心規則**

> 借來的東西，用完必須原封不動。透過 `&T` 或 `&mut T` 可以看、再借、copy、clone，不能 move。

`let x = 某個東西;` 是 copy 還是 move，看**被拿出來的那一塊**的型別有沒有 `Copy`，不看外層的 struct。

```rust
struct Player {
    name: String, // 沒有 Copy
    hp: u32,      // 有 Copy
}

impl Player {
    fn a(&self) {
        let h = self.hp;           // ✅ 拿的是 u32，copy。Player 本身沒有 Copy 也沒關係
        let r = &self.name;        // ✅ 再借
        let c = self.name.clone(); // ✅ 另外做一份
        let n = self.name;         // ❌ E0507: cannot move out of `self.name` which is behind a shared reference
        let p = *self;             // ❌ E0507: cannot move out of `*self` which is behind a shared reference
    }
}
```

| 透過 `&self` 做的事 | 可以嗎 | 做完之後原本的 instance |
|---|---|---|
| 看（`match`、比較、印） | ✅ | 沒變 |
| 再借：`&self.name` | ✅ | 沒變 |
| copy：`self.hp` | ✅ | 沒變，多了一份 |
| clone：`self.name.clone()` | ✅ | 沒變，多了一份 |
| move：`self.name`、`*self` | ❌ E0507 | 會留下一個被掏空的位置 |

**關鍵細節**

- `*self` 是「借條指的那個位置」，不是「把東西拿過來」。寫 `*` 本身不搬任何東西。`self.name` 就是 rustc 自動補成的 `(*self).name`，所以 `&self` 的 fn 一直都在用 `*`。
- 能不能釋放 instance 在簽名就決定了：`self` 有擁有權，`&self` / `&mut self` 沒有。body 裡怎麼寫都改變不了。
- copy 和 move 在機器層是同一個動作（照抄值自己的 byte），差在抄完之後原本那份還准不准用。copy 之後原本那份照樣能用，所以不違反「原封不動」；move 之後原本那份失效，所以違反。
- `&mut self` 可以改，一樣不能 move（E0507，這次訊息是 behind a mutable reference）。要拿走就得同時放一個回去：`std::mem::take(&mut self.name)` 把內容拿走，原位留下空字串。

**`match self` 與 `match *self`**

| | `match self` | `match *self` |
|---|---|---|
| 被 match 的東西的型別 | `&T`（借條） | `T`（借條指的位置） |
| arm 綁出來的變數 | 自動變成 `&欄位` | 欄位本身，等於要拿出來 |
| 欄位是 `Copy`（`usize`） | `&usize` | `usize`，複製一份 |
| 欄位不是 `Copy`（`String`） | `&String` | ❌ E0507 |
| arm 沒有綁變數（`Op::Add`、`E::Bad(_)`） | 沒差 | 沒差 |

```rust
match *self {
    E::Bad(t) => …    // ❌ t 要成為 String，等於搬走
    E::Many(n) => …   // ✅ n 是 usize，複製
    E::Bad(_) => …    // ✅ 沒有綁變數，沒有東西被搬
}
```

- `match` 只是去看那個位置。會不會 move 由 arm 綁了什麼決定，不是 `match` 後面那個 `*`。
- `match *self` 是舊寫法。現在的習慣是 `match self`，綁出來的變數自動是 reference。

### 思維鏈：一個 `String` 走過 move、borrow、clone、drop

```rust
fn look(s: &String) {}
fn take(s: String) {}

fn main() {
    let a = String::from("hi");   // t0
    let b = a;                    // t1
    look(&b);                     // t2
    let c = b.clone();            // t3
    take(b);                      // t4、t5
}                                 // t6
```

```
時間  動作              照抄的 byte          heap                  擁有者
t0   String::from      從執行檔複製 2 byte   配置 P                a 擁有 P
t1   let b = a         24 byte              不動                  b 擁有 P，a 失效
t2   look(&b)          8 byte 位址          不動                  b 擁有 P（沒變）
     look 結束                               不釋放
t3   b.clone()         24 byte + 2 byte     配置 Q                b 擁有 P，c 擁有 Q
t4   take(b)           24 byte 到 take 的 frame  不動             s 擁有 P，b 失效
t5   take 結束                               釋放 P                s 離開作用域
t6   main 結束                               釋放 Q                c 離開作用域
                                                                  a、b 已失效，什麼都不做

合計：配置 2 次、釋放 2 次；"hi" 最多同時存在 3 份（執行檔、P、Q）。
```

### 陷阱清單

1. ❌ 編譯器不知道大小的東西會被放到 heap → ✅ rustc 從不自動配置 heap，不定大小的型別只准透過帶長度的指標使用 → 為什麼：配置必須在程式碼裡看得見，放進 heap 一定是有人呼叫了 allocator。
2. ❌ `String` 在 heap 是因為動態大小、陣列在 stack 是因為大小固定 → ✅ 位置看「怎麼建立的」：`String::from` 呼叫了 allocator、`let buf` 是區域變數 → 為什麼：大小固定只是放 stack 的資格，`Box::new([1, 2])` 就把固定大小的東西放進 heap。
3. ❌ 把值傳給別的函式，它就會離開 stack → ✅ 傳值只是在對方的 stack frame 再放一份 → 為什麼：函式呼叫只動 stack，會動 heap 的只有 allocator。
4. ❌ 陣列能複製、`String` 不能，是因為 stack 能抄、heap 不能抄 → ✅ 機器動作相同，都是照抄值自己的 byte；差在抄完會不會出現兩個擁有者 → 為什麼：`String` 的 24 byte 只是指標，照抄會讓兩個變數都去釋放同一塊 heap。
5. ❌ copy 還是 move，看是不是基本型別 → ✅ 看型別有沒有實作 `Copy` → 為什麼：`&mut T`、`[String; 2]` 是基本型別卻不是 `Copy`；自訂 struct 加了 derive 就是 `Copy`。
6. ❌ move 之後原變數等釋放完就能再用 → ✅ 原變數永久失效，只有重新賦值才能再用；釋放由新擁有者做一次 → 為什麼：原變數不擁有任何東西了，也不會被 drop。
7. ❌ move 是 heap 上的權力轉移 → ✅ 任何不是 `Copy` 的型別以值使用都是 move，沒用到 heap 的 struct 也一樣 → 為什麼：擁有權管的不只記憶體，還有檔案、鎖、連線。
8. ❌ `Copy` 是一個 method，自己寫一個 copy method 就能避開 move → ✅ `Copy` 是空的 trait 標籤，rustc 只認這個標籤 → 為什麼：複製動作是 rustc 產生的，作者無法自訂，否則到處都是隱藏的程式碼在跑。
9. ❌ 欄位不能 `Copy` 是因為它用到 heap → ✅ 是因為它擁有需要釋放的東西 → 為什麼：`&str` 指向 heap 卻是 `Copy`，`File` 沒用 heap 卻不是。
10. ❌ move 等於 borrow；或 borrow 是傳址、move 是傳值 → ✅ 兩者相反：move 交出擁有權，borrow 不變。傳值包含 `Copy` 與 move 兩種結果，borrow 則是把位址這個值傳過去 → 為什麼：borrow 之後由原擁有者釋放，move 之後由新擁有者釋放。
11. ❌ `&Box<T>` 指向 heap → ✅ `&X` 指向 X 所在的位置，`Box` 變數通常在 stack → 為什麼：`&` 不管 X 裡面又指向哪裡。
12. ❌ struct 本體都在 stack，member 再看型別決定放哪；放進 heap 容器後就變成一整塊連續 → ✅ 欄位永遠排在本體那一塊裡，本體在哪看你怎麼放；`String` 欄位的內容永遠是另外一塊 → 為什麼：值的大小必須固定，容器才能算出第 i 個元素的位置。
13. ❌ 在 heap 裡的 struct 不能 `Copy`，`Copy` 是 stack 情境專用 → ✅ `Copy` 是型別的性質，跟值在哪無關 → 為什麼：照抄一塊固定大小的 byte，來源和目的地在哪都可以。
14. ❌ 複製是賦值才發生，所以複製出來的一定在 stack → ✅ 那一份在目的地；`v[0] = a`、`*boxed = a` 的目的地在 heap → 為什麼：任何以值使用的地方都會複製，不只 `let`。
15. ❌ move 等於 clone，或 move 傳的一定是位址 → ✅ move 只抄值自己的 byte，heap 內容不動；clone 才配置新的一塊 → 為什麼：1 GB 的 `String` move 只抄 24 byte。
16. ❌ 一直 copy 會 stack overflow → ✅ stack frame 編譯期就固定，重複複製只是往同一格寫 → 為什麼：overflow 來自呼叫太深或單一區域變數太大。
17. ❌ clone 一定是最吃資源的 → ✅ 看型別：`String` 深複製，`Rc` 只加計數 → 為什麼：`clone` 是作者寫的 method，不是固定動作。
18. ❌ `match *self` 會把 instance 拿過來，fn 結束後釋放 → ✅ `*self` 只是借條指的位置，`match` 只是看；`&self` 的 fn 無論怎麼寫都不會釋放 instance → 為什麼：釋放是擁有者的事，有沒有擁有權在簽名（`self` vs `&self`）就決定了。
19. ❌ `&self` 的 fn 裡不能用 `*self` 取值，只能操作位址 → ✅ `*self` 可以用，`self.field` 本來就是 `(*self).field`；不能做的只有 move → 為什麼：被禁止的是「搬走」這個動作，不是 `*` 這個符號。
20. ❌ 透過借用不能 copy 也不能 move → ✅ copy 可以，只有 move 不行 → 為什麼：copy 之後原本那份原封不動，move 之後原本那份失效，擁有者回來會拿到壞掉的值。
21. ❌ 能不能從借用裡拿出來，看這個 struct 有沒有實作 `Copy` → ✅ 看被拿出來的那一塊的型別：`self.hp`（`u32`）可以，`self.name`（`String`）不行，跟 `Player` 有沒有 `Copy` 無關 → 為什麼：欄位各有各的型別，`Copy` 是型別的性質。

---

## 5. Process 的記憶體區域與 kernel 的角色

> 回頭補的一節。接第 3 節（stack 與 heap）往外補上其他區域，接第 11 節（user mode ↔ kernel mode）從記憶體這一側看 kernel 什麼時候進場。`static` / `const` 與 ELF 區段在 [note 04 §10](04-borrow-exclusivity-lifetime-elf.md) 走過一次，這裡補上依型別分區、存取成本與三層保護。標 **補強** 的 blockquote 是整理進 repo 時加註的，不在原本這條線的對話裡；註明「Mac 實測」的是 rustc 1.87、Apple Silicon、`-O` 跑出來的。

### 一句話版本

一個 process 的 stack、heap、`.rodata`、`.data`、`.bss` 都是同一種 RAM，差別只在「空間怎麼來、位址怎麼找、誰負責歸還」。kernel 只在事前鋪好 page table，平常的讀寫是你的 thread 在 user mode 執行指令、由 CPU 的 MMU 查表完成，kernel 不在場。

### 分層模型（骨架）

```
rustc / linker    決定每樣東西進哪個區段，寫進執行檔
kernel            exec 時建立位址空間、設好 page table 與各頁權限
                  之後只在 syscall、例外、中斷時進場
CPU（MMU）        每一次記憶體存取：查 page table、檢查權限、換成實體位址
你的 thread       在 user mode 執行 load / store 指令
allocator（std）  user mode 的程式碼，管理 heap，不夠時才發 syscall
```

### 位址空間裡的區域

**核心規則**：這些區域是同一個 process 位址空間裡的不同範圍，全部都是 RAM。位置由「怎麼建立的」決定。

| 區域 | 放什麼 | 可寫 | 什麼時候出現 | 什麼時候消失 |
|---|---|---|---|---|
| `.text` | 機器碼 | 不可 | 程式載入 | process 結束 |
| `.rodata` | 字串字面值、不可變的 `static` | 不可 | 程式載入 | process 結束 |
| `.data` | 有初值的可變全域資料 | 可 | 程式載入 | process 結束 |
| `.bss` | 初值是 0 的可變全域資料 | 可 | 程式載入 | process 結束 |
| heap | `Box`、`Vec`、`String` 的內容 | 可 | 有人呼叫 allocator | 擁有者離開作用域 |
| stack | 區域變數、參數、回傳位址 | 可 | 進入函式 | 函式回傳 |

**關鍵細節**

- 「可不可寫」是 kernel 載入執行檔時設在每一頁上的權限，由 CPU 在每次存取時檢查。
- macOS 的執行檔格式區段名稱不同（`__TEXT`、`__DATA` 等），概念相同。
- `.rodata` 既不是 stack 也不是 heap。

### `const` 與 `static`

**核心規則**：`const` 沒有儲存位置，編譯期把值代入每個使用處，最後嵌在機器碼裡。`static` 有固定位址，整個程式一份，活到 process 結束。

| | `const` | `static` |
|---|---|---|
| 有固定位址嗎 | 沒有 | 有 |
| 執行期存在嗎 | 不存在，只剩嵌在指令裡的值 | 存在 |
| 能改嗎 | 不能 | 搭配 `Atomic`、`Mutex` 可以 |
| 適合 | 單純的常數值 | 需要「同一份」的東西：計數器、快取、設定 |

| 寫法 | 位置 | 能改嗎 |
|---|---|---|
| `const MAX: u32 = 5;` | 無，代入使用處 | 不能 |
| `static MAX: u32 = 5;` | `.rodata` | 不能 |
| `static N: AtomicU32 = AtomicU32::new(0);` | `.data` 或 `.bss` | 能，`fetch_add`、`store` |
| `static L: Mutex<Vec<u32>> = Mutex::new(Vec::new());` | `.data` 或 `.bss` | 能，先 `lock()` |
| `static C: LazyLock<T> = LazyLock::new(\|\| ..);` | `.data` 或 `.bss` | 第一次存取時初始化一次 |
| `static mut N: u32 = 0;` | `.data` 或 `.bss` | 能，但每次都要 `unsafe`，不建議 |

**關鍵細節**

- `static` 的初值必須編譯期算得出來；需要配置 heap 或執行期才知道的，用 `LazyLock`。
- `static` 的型別必須能安全地被多條 thread 同時存取，因為任何 thread 都拿得到它。
- `static` 永遠不會被釋放，process 結束時也不執行它的 `drop`。
- rustc 依型別決定區段：型別允許「透過 `&` 修改」的，放可寫區；否則放 `.rodata`。
- interior mutability（內部可變性）：`Atomic`、`Mutex` 這類型別允許透過共享的 `&` 修改，因為它們自己保證安全。所以 `&T` 精確的意思是「共享的參考」，不是「一定唯讀」。
- `static` 關鍵字和 `'static` lifetime 是兩件事：前者宣告全域變數，後者是說「這個參考永遠有效」。

> **補強**：`const`「沒有儲存位置」精確的意思是「不保證有唯一的位址」，不是「內容一定不在記憶體裡」。小的值會變成指令裡的立即數；但 `const S: &str = "..";` 的字串內容、大陣列，或對它取參考（`&MAX`）時，rustc 還是會把內容放進唯讀區，只是每個使用處可以各拿一份，也可以被合併。Mac 實測：`&MAX` 印得出位址，而且跟另一個 `static S_RO: u32 = 5;` 是同一個位址（內容相同被合併了）。
>
> 會預測錯的是 `const` 配 interior mutability：每個使用處都是一份新的值。Mac 實測：
>
> ```rust
> const CN: AtomicU32 = AtomicU32::new(0);
>
> CN.fetch_add(1, Ordering::SeqCst);
> CN.fetch_add(1, Ordering::SeqCst);
> CN.load(Ordering::SeqCst)          // 0：三行各用各的一份
> ```
>
> 同樣三行把 `const` 換成 `static` 得到 2。計數器、鎖這類要「同一份」的東西只能用 `static`；clippy 的 `declare_interior_mutable_const` 會抓這個寫法。

### 存取成本：差別不在區域

**核心規則**：讀寫 stack、heap、static 的一個 byte，速度沒有差別。記憶體存取也不是 IO。

| | 位址怎麼得到 | 取得與歸還空間的成本 | 讀一個值 |
|---|---|---|---|
| `const` | 沒有位址 | 無 | 通常不用讀記憶體 |
| stack 變數 | stack 指標加固定位移 | 幾乎為零，移動指標 | 一次存取 |
| `static` | link 時就固定 | 零，載入就在 | 一次存取 |
| heap 上的資料 | 先讀出指標再跟過去 | 呼叫 allocator，最貴 | 兩次：指標、內容 |

真正造成速度差異的是：

| 原因 | 說明 |
|---|---|
| 配置與釋放 | heap 要呼叫 allocator，stack 和 static 不用 |
| 多跳一次 | 透過指標找資料，多一次記憶體存取 |
| CPU cache | 最近用過的記憶體讀起來快，stack 頂端幾乎總在 cache 裡 |
| 同步的成本 | `Atomic`、`Mutex` 為了多 thread 安全要付額外代價 |

> **補強**：「link 時就固定」固定的是 `static` 在執行檔映像裡的位移，不是絕對位址。現在的執行檔預設位址無關（PIE），kernel 每次載入都把整個映像放在不同的起點（ASLR）。Mac 實測同一個 `static` 連跑兩次：`0x1007d4fc8`、`0x100c64fc8`，尾端 `4fc8` 相同，前面不同。指令用「相對於目前這條指令的位移」找它，不必先查表，所以「一次存取」的結論不變；會錯的預測只有「這個位址每次執行都一樣」。

### kernel 什麼時候在場

**核心規則**：kernel 事先鋪好地圖（page table），之後一般的記憶體讀寫完全不經過它。進入 kernel 只有三種方式。

| 進入方式 | 誰觸發 | 例子 |
|---|---|---|
| syscall | 你的程式主動要求 | 讀檔、網路、要更多記憶體、建立 thread |
| 例外 | 你的指令執行時出狀況，CPU 強制跳進去 | page fault、除以零、寫唯讀頁 |
| 中斷 | 外部硬體，跟你的程式在做什麼無關 | 計時器、網卡 |

| 動作 | 進 kernel 嗎 |
|---|---|
| 讀寫已映射的記憶體（stack、heap、static 都一樣） | 不進 |
| 函式呼叫、建立區域變數 | 不進 |
| `Box::new`、`Vec::push` 大多數時候 | 不進，allocator 從手上已有的記憶體切一塊 |
| allocator 手上不夠，要新的頁 | 進，syscall |
| 第一次碰到一頁還沒實際配給的記憶體 | 進，page fault |
| 寫入 `.rodata` | 進，例外，process 被終止 |
| 讀檔、網路、印到螢幕 | 進，這些才是 IO |

**關鍵細節**

- 「進入 kernel」是同一條 thread 的 CPU 從 user mode 切到 kernel mode，改跑 kernel 的程式碼，做完切回來。kernel 一直都在記憶體裡，不需要啟動，也沒有另一個角色被叫醒。
- exec 時 kernel 多半只登記「這段位址是你的」，實體 RAM 等第一次碰到那一頁才給。
- 這樣設計的原因：進出 kernel 一次的成本是一次記憶體存取的上百倍。所以 kernel 管低頻的「誰能用哪些記憶體」，硬體負責每一次存取的檢查與轉換。
- 比喻：kernel 是發地契的機關，MMU 是門口自動查驗地契的閘門。在自己的地上進出不用去機關，要新的地或闖進別人的地才會驚動它。

> **補強**：「上百倍」是跟已經在 CPU cache 裡的那種存取比。拿第 11 節的數量級階梯對：L1 命中約 1 ns、syscall 約 0.5～1 µs，差 500～1000 倍；跟沒命中 cache、真的要到 RAM 拿的那一次（約 100 ns）比，只差 5～10 倍。方向不變：每次存取都進 kernel 付不起。

### `.rodata` 的保護與內容來源

**核心規則**：正常情況下沒有任何人寫 `.rodata`。「寫入 `.rodata`」是錯誤情境，三層各擋一次。

| 層 | 怎麼擋 |
|---|---|
| rustc | 編譯期：不讓你透過 `&` 去改不可變的資料 |
| CPU（MMU） | 執行期：那一頁沒有寫入權限，指令執行不了，觸發例外 |
| kernel | 接到例外後終止 process |

| 時間點 | 誰 | 對 `.rodata` 做什麼 |
|---|---|---|
| 編譯、link | rustc、linker | 把內容寫進執行檔 |
| 載入 | kernel | 把內容放進 RAM，權限設成只讀 |
| 執行期 | 你的程式 | 只能讀 |
| 執行期，有人硬要寫 | CPU 加 kernel | 擋下並終止 |

載入時把內容放進 RAM 的是 kernel，在 kernel mode 進行。頁的只讀權限管的是你的程式在 user mode 的存取。

> **補強**：「不可變的 `static` 在 `.rodata`、內容由 kernel 載入」有一類例外：內容裡有指標的不可變 `static`，例如 `static S: &str = "hi";`。位址無關的執行檔要到載入後才知道 `"hi"` 的絕對位址，`S` 那 16 byte 裡的指標得在啟動時填上，所以它不能放在從頭到尾都唯讀的頁。
>
> | `static` | Mac 實測落在哪 | 那一段的權限 |
> |---|---|---|
> | `static S_RO: u32 = 5;` | `__TEXT,__const` | 從載入起就唯讀 |
> | `static S: &str = "hi";`（`S` 本身的 16 byte） | `__DATA_CONST,__const` | 載入時可寫，標記為「位址填完後改成唯讀」 |
> | `"hi"` 的內容 | `__TEXT` 裡的唯讀區 | 從載入起就唯讀 |
>
> Linux 的對應是 `.data.rel.ro` 加 RELRO：動態載入器 `ld.so` 在 user mode 把位址寫進去，再用 `mprotect` 這個 syscall 請 kernel 收回寫入權限（這台機器沒有實測；重定位那一步在經驗 01 §7）。macOS 由 `dyld` 安排同一件事，實際動手的是 `dyld` 還是 kernel 沒有查證。
>
> 所以「誰寫過唯讀資料」完整是三個時間點：linker 寫進檔案、kernel 載入、啟動時填位址。`main` 開始之後才是誰都不能寫，上面那張「三層各擋一次」的表講的是這之後的事。

### 思維鏈：一個 process 從 exec 到讀寫記憶體

```
時間  誰 / mode             發生什麼
t0   父 process            發出 exec syscall
t1   kernel                建立位址空間；把執行檔的 .text、.rodata、.data 映射進來，
                           登記 .bss 與 stack；設好各頁權限。實體 RAM 多半還沒給
t2   kernel → user         跳到進入點，std 的啟動碼執行後呼叫 main
t3   你的 thread / user    讀字面值 "hi"：load 指令 → MMU 查表 → 這一頁還沒載入
                           → CPU 觸發 page fault
t4   同一條 thread / kernel 把那一頁的內容備妥、補上映射，回 user mode 重跑那條指令
t5   你的 thread / user    let x = 5;  移動 stack 指標、store。MMU 查表通過，kernel 不在場
t6   你的 thread / user    COUNTER.fetch_add(..)  對 .bss 裡的位址執行原子指令，kernel 不在場
t7   你的 thread / user    String::from("hi")  呼叫 allocator
t8   allocator / user      手上有空間：切一塊回傳，不進 kernel
                           手上不夠：發 syscall 向 kernel 要新的位址範圍
t9   你的 thread / user    把 2 byte 從 .rodata 複製到 heap
                           若這一頁是第一次碰：page fault → kernel 配一頁實體 RAM
t10  任何時刻              計時器中斷 → CPU 強制進 kernel → 排程器可能換別的 thread 上來
t11  你的 thread           println!  發 write syscall → 進 kernel → 回 user mode
t12  main 結束             exit syscall → kernel 收回整個位址空間，static 不執行 drop
```

> **補強**：t1 到 t2 之間，動態連結的執行檔還有一段。kernel 把執行檔和動態載入器都映射進來之後，先跳到載入器；載入器在 user mode 把需要的函式庫映射進來、做重定位，才跳到程式的進入點（經驗 01 §7）。時間軸寫的「kernel 直接跳到進入點」是靜態連結的情況；macOS 上的執行檔都會經過 `dyld`。

### 陷阱清單

1. ❌ `static` 都寫在 `.rodata` → ✅ 不可變的才在 `.rodata`；`AtomicU32`、`Mutex` 這種可修改的在 `.data` 或 `.bss` → 為什麼：rustc 依型別是否允許透過 `&` 修改來決定區段，`.rodata` 的頁沒有寫入權限。
2. ❌ thread 能直接用 `static`，是因為它在 `.rodata` → ✅ 是因為它活到 process 結束，擁有者不會先消失 → 為什麼：能不能借進 thread 看的是擁有者活多久，不是資料在哪。
3. ❌ 沒寫 `mut` 就不可能被修改 → ✅ `Atomic`、`Mutex` 可以透過 `&self` 修改（interior mutability） → 為什麼：這些型別自己保證多 thread 下的安全，`&` 的意思是共享而不是唯讀。
4. ❌ `const` 不在 static 區，所以在 stack → ✅ `const` 哪裡都不在，編譯期把值代入使用處、嵌進機器碼 → 為什麼：執行期沒有它這個東西；`let m = MAX;` 在 stack 上的是 `m` 這份副本。
5. ❌ `static` 不在 stack，所以存取比較慢；記憶體存取是一種 IO → ✅ 所有區域都是同一種 RAM，讀寫一個值的速度相同；IO 指磁碟、網路 → 為什麼：速度差異來自配置成本、多跳一次指標、cache、同步，不是區域。
6. ❌ 讀寫記憶體都要透過 kernel → ✅ 一般讀寫是 user mode 的指令加上 MMU 查表，kernel 不參與 → 為什麼：進出 kernel 的成本是記憶體存取的上百倍。
7. ❌ 需要時會「啟動 kernel」 → ✅ 是「進入 kernel」：同一條 thread 切到 kernel mode 跑 kernel 的程式碼 → 為什麼：kernel 一直都在，不是一個等著被叫醒的角色。
8. ❌ process 啟動時 kernel 把記憶體配好，之後就不再需要它 → ✅ 啟動時多半只登記位址範圍，實體 RAM 等第一次碰到才給，那時會進 kernel 一次 → 為什麼：page fault 不是你的程式主動要求的，是 CPU 觸發的。
9. ❌ 只有索取資源時才會進 kernel → ✅ 有三種：syscall、例外、中斷 → 為什麼：計時器中斷讓 kernel 就算面對不發任何 syscall 的程式也拿得回 CPU。
10. ❌ 程式載入時是在「寫入 `.rodata`」，所以它其實可寫 → ✅ 載入時把內容放進 RAM 的是 kernel，在 kernel mode 做；頁的只讀權限管的是 user mode 的存取 → 為什麼：之後任何 user mode 的寫入都會被 MMU 擋下。

---

## 6. 佔多少空間：型別大小、enum、函式與 stack frame

> 回頭補的一節。起點是動手練習時問的「`Command` 這個 enum 多大？`impl` 裡都是函式，函式有大小嗎？」，一路拆出三種不同的「大小」。接第 4 節（值自己的 byte、stack frame 與複製次數）往下補型別佈局與 frame 的邊界，接第 5 節（位址空間的區域）補上 `.text`。標「實測」的數字是 rustc 1.97、Linux x86-64 跑出來的；frame 的 160 byte 是 debug build。

### 一句話版本

「多大」有三個互不相干的答案：一個值佔多少 byte（看型別定義裡的欄位）、一個函式的機器碼多長（看函式內容，住在 `.text`）、一次函式呼叫要多少 stack（frame，編譯期算好的常數）。`enum` 和 `struct` 都只是在描述資料的形狀，`impl` 裡的函式不佔值的空間。

### 分層模型（骨架）

```
rustc（編譯期）   算出每個型別的 size / alignment、enum 的 tag 放哪
                  算出每個函式的 frame 多大，把數字寫死進函式開頭的指令
                  把每個函式編成機器碼
linker            把機器碼收進 .text，字面值收進 .rodata
kernel            exec 時把 .text 映射成「可讀可執行」；給 thread 一塊 stack 區域
                  之後 stack 裡怎麼切 frame，它不知道也不管
CPU               stack pointer 暫存器記著 stack 用到哪；MMU 以 page 為單位查表
你的 thread       在 user mode 執行函式開頭那條「stack pointer 減 N」→ frame 出現
```

### 三種「大小」

**核心規則**：資料和程式碼住在不同地方。問大小之前先分清楚問的是哪一個。

| 問的是 | 是什麼 | 由什麼決定 | 住在哪 | 有幾份 |
|---|---|---|---|---|
| 值的大小 | `size_of::<T>()` | 型別定義裡的欄位 | 那個值被放在哪就在哪（stack、heap、static 區） | 建立幾個值就幾份 |
| 函式的機器碼 | 一串 CPU 指令 | 函式內容寫了多少 | `.text` | 整個程式一份 |
| stack frame | 一次呼叫專用的 stack 空間 | 函式裡有哪些變數與暫時值 | 那條 thread 的 stack | 有幾次呼叫還沒返回就幾份 |

```
執行檔（載入後不變）                STACK（隨呼叫變化）
┌──────────────────┐             ┌───────────────────┐
│ .text            │             │ main 的 frame      │
│   parse 的機器碼  │             │   bag、lines …     │
│   execute 的機器碼│             ├───────────────────┤
│   main 的機器碼   │             │ execute 的 frame   │
├──────────────────┤             │   cmd: Command 32  │ ← 一個值被放進了這個 frame
│ .rodata          │             │   bag、idx …       │
│   "add" "remove" │             └───────────────────┘
└──────────────────┘
```

**關鍵細節**

- 三者之間沒有固定的大小關係。唯一成立的是「容器不小於內容物」：一個 frame ≥ 直接放在它裡面的所有值的總和。
- 一個 `Command` 也可以在 heap（`Vec<Command>` 的元素），那時它和任何 frame 的大小都無關。
- 機器碼在 `.text`（可讀、可執行），字面值在 `.rodata`（只可讀）。兩者都來自執行檔、都不可寫，差別在一個給 CPU 執行，一個給程式讀取。

### 型別的大小：欄位、對齊、padding

**核心規則**：`size_of::<T>()` 是編譯期常數，只算「值自己的 byte」。算法是欄位加總，再補到對齊的整數倍。

1. 型別的對齊（alignment）= 欄位中最大的對齊
2. 型別的大小必須是對齊的整數倍，不足的補空白（padding）

| 型別 | 欄位加總 | 對齊 | `size_of`（實測） |
|---|---|---|---|
| `Item { name: ItemName, weight: u32, value: u32 }`，`ItemName` 是無欄位 enum | 1 + 4 + 4 = 9 | 4 | 12 |
| 同上，`name: &str` | 16 + 4 + 4 = 24 | 8 | 24 |
| 同上，`name: String` | 24 + 4 + 4 = 32 | 8 | 32 |
| `Player { name: String, hp: u32, capacity: u32, bag: Vec<Item> }` | 24 + 4 + 4 + 24 = 56 | 8 | 56 |

**關鍵細節**

- `Vec` 放再多元素，`size_of::<Player>()` 都是 56：算進去的只有 `Vec` 的 ptr、len、cap 三個欄位，元素在 heap。
- `Vec` 只有**一個**指標，指向**一塊**連續空間，元素一個挨一個。相鄰兩個元素的位址差就是 `size_of::<Item>()`（實測差 12）。
- `&str` 固定 16 byte（位址 + 長度），和指向的字串多長無關。`String` 多一個容量，24 byte。

### enum：每個 variant 是一個 struct，外加 tag

**核心規則**：`enum` 的意思是「這個值是下面幾種 struct 的其中一種」。大小 = 最大的那個 variant + 記錄「現在是哪一種」的 tag。

> **tag 還沒正式學到。** 這一節只把它當成「enum 裡記錄現在是哪個 variant 的那個 key」來推算大小。它的正式名稱（discriminant）、實際是什麼型別、rustc 把它放在哪、怎麼指定（`#[repr]`）或讀出來（`std::mem::discriminant`），都還沒挖。下面的佈局圖是推算用的概念圖，不是 rustc 實際的佈局。

variant 的三種寫法和 struct 的三種寫法一一對應：

| 形式 | struct | enum 的 variant | 欄位怎麼取 |
|---|---|---|---|
| 具名欄位 | `struct P { x: i32, y: i32 }` | `Move { x: i32, y: i32 }` | 用名字 |
| tuple 形式 | `struct P(i32, i32);` | `Add(String, u32)` | 用位置 |
| 沒有欄位 | `struct Marker;` | `List`（等價於 `List {}`） | 沒有東西可取 |

```rust
enum Command {
    Add(String, u32),   // 24 + 4 = 28   ← 最大
    Remove(String),     // 24
    List,               // 0
    Quit,               // 0
    Clear,              // 0
}
// 最大 28 → 對齊 8 → 32。tag 只要 1 byte，補出來的 4 byte 放得下，不另外佔空間。
// size_of::<Command>() == 32（實測）
```

```
                 0        8        16       24    28   29     32
Add("sword",2)   [ ptr    | len=5  | cap=5  | 2   |tag | 補白  ]
Remove("sword")  [ ptr    | len=5  | cap=5  |沒用到|tag | 補白  ]
List             [ 沒用到                          |tag | 補白  ]
```

（tag 實際放在哪個位置由 rustc 決定，這是用來推算大小的概念圖。）

**關鍵細節**

- `Add(String, u32)` 的括號是**欄位**，不是參數。它和 `Add { name: String, qty: u32 }` 是同一件事，只差欄位沒取名。
- 值是 `Command::List` 的變數仍然佔 32 byte。`List` 自己帶的資料是 0，但變數的型別是 `Command`，同型別的值必須一樣大，才放得進 `Vec<Command>` 這種每格等寬的容器。
- 每個 variant 都沒有欄位的 enum 只剩 tag：`enum ItemName { Sword, Shield, Anvil, Potion }` 是 1 byte，本質上是一個小整數。
- tag 有兩個地方可以放，都不會讓 enum 變大：對齊補出來的空 byte，或某個欄位裡不會出現的值（下一小節的 niche）。兩種都沒有時才需要另外佔空間。實測（把 `String` 換成同樣 24 byte、沒有任何空位的 `[u64; 3]`）：

  | 型別 | 欄位裡有空位 | 有補白 | `size_of` |
  |---|---|---|---|
  | `Command { Add(String, u32), … }` | 有 | 有 | 32 |
  | `NoNiche { Add([u64; 3], u32), … }` | 沒有 | 有（4 byte） | 32 |
  | `NoSlack { Add([u64; 3], u64), … }` | 沒有 | 沒有 | 40（tag 1 + 對齊 7） |
- 執行期的 `match` 就是讀 tag、跳到對應的 arm。
- `Option<T>` 只是 std 裡一個普通的 enum（`None` / `Some(T)`），適用同一套規則。

### niche：借一個不會出現的值

**核心規則**：「型別佔多少空間」和「型別有幾種合法的值」是兩件事。前者能表示的組合數減掉後者，剩下的就是空位（niche）。`Option<T>` 如果能在 `T` 裡找到一個空位，就拿它當 `None`，不用自己開 tag；找不到才另外開。借的是「不會出現的值」，不是「多餘的 byte」。

判斷只要問一句：**這個型別裡，有沒有任何一個欄位存在「合法的值永遠不會是它」的 bit 組合？**

#### enum 的 tag 本身就是空位：兩層 enum 共用一個 tag

`Command` 是 enum，裡面有一個 tag 記「現在是哪個 variant」。`Option<Command>` 也是 enum，照理也需要一個 tag 記「`Some` 還是 `None`」。但 `Command` 的 tag 是 1 byte、只用了 5 個值，`Option` 就不另外開，直接借第 6 個值：

| tag 的值 | 代表 |
|---|---|
| 0 | `Some(Add(…))` |
| 1 | `Some(Remove(…))` |
| 2 | `Some(List)` |
| 3 | `Some(Quit)` |
| 4 | `Some(Clear)` |
| 5 | `None` |
| 6 到 255 | 還沒用到 |

不共用的話會是 `[Option 的 tag | Command 32 byte]` = 33，對齊後 40。合併成一個 tag 省下這 8 byte，這個動作就是 niche optimization。

所以 `Option<Command>` 是 32 要分兩步看：

| 步驟 | 問題 | 靠什麼 |
|---|---|---|
| `Command` = 32 | `Command` 自己的 tag 放哪 | 對齊補出來的空 byte |
| `Option<Command>` = 32 | `None` 怎麼表示 | tag 那個 byte 裡沒用到的值 |

#### 借得到的型別（`Option<T>` 和 `T` 一樣大，實測）

| 類別 | 型別 | 大小 | 借的是什麼 |
|---|---|---|---|
| 不可能是空的指標 | `&T`、`&mut T`、`Box<T>`、`Rc<T>`、`Arc<T>`、`NonNull<T>`、`fn()` | 8 | 位址 0 |
| 胖指標 | `&str`、`&[T]` | 16 | 指標欄位的 0 |
| 內含上述指標的容器 | `String`、`Vec<T>` | 24 | 內部欄位不會出現的值 |
| | `HashMap<K, V>` | 48 | 同上 |
| 用不滿的基本型別 | `bool` | 1 | 2 到 255 |
| | `char` | 4 | 超出 Unicode 範圍的值 |
| 明確排除 0 的整數 | `NonZeroU32` 等 | 4 | 0 |
| enum 的 tag | `ItemName`、`Command`、`Ordering`，任何沒把 tag 用滿的 enum | 1 / 32 / 1 | tag 沒用到的值 |
| 有限制範圍的欄位 | `Duration` | 16 | 奈秒欄位不會超過 10 億 |
| | `File` | 4 | fd 不會是 -1 |
| 含有以上任一欄位的複合型別 | `(u32, bool)`、`struct { a: u32, b: bool }`、`[bool; 2]` | 8 / 8 / 2 | 裡面那個欄位的空位 |

#### 借不到的型別（每一種 bit 組合都是合法的值，實測）

| 類別 | 型別 | `T` | `Option<T>` |
|---|---|---|---|
| 整數 | `u8` | 1 | 2 |
| | `u32`、`i32` | 4 | 8（tag 1 + padding 3 + 4） |
| | `u64`、`usize` | 8 | 16 |
| 浮點數 | `f64` | 8 | 16 |
| 原始指標 | `*const T` | 8 | 16 |
| 只由上述組成的複合型別 | `[u8; 4]` | 4 | 5 |
| | `(u32, u32)`、`struct { a: u32, b: u32 }` | 8 | 12 |
| 空型別 | `()` | 0 | 1 |

#### 空位會被用掉

| 型別 | 大小 | 說明 |
|---|---|---|
| `Option<&u32>` | 8 | 借了位址 0 |
| `Option<Option<&u32>>` | 16 | 指標只有 0 這一個空位，裡面那層用掉了，外面那層只好自己開 tag |
| `Option<bool>` | 1 | 借了 2 |
| `Option<Option<bool>>` | 1 | `bool` 有 254 個空位，再借一個也夠 |

```
                 指標（8 byte）            長度（8 byte）
Some("hi")       00 10 00 00 00 00 00 00   02 00 00 00 00 00 00 00
None             00 00 00 00 00 00 00 00   ?? ?? ?? ?? ?? ?? ?? ??
                 └ 全 0 代表 None ────────┘   └ 不看 ────────────────┘
```

**關鍵細節**

- 複合型別只要**其中一個欄位**借得到，整個就借得到：`(u32, bool)` 靠裡面的 `bool`；`(u32, u32)` 兩個欄位的每個值都合法，借不到。
- 空位不限於指標。指標的 0、`bool` 的 2 到 255、enum 的 tag 沒用到的值，都是同一回事。
- `&T` 和 `*const T` 同樣是 8 byte 的位址。`*const T` 允許是 null，0 是合法值，所以借不到。差別完全來自型別的規則，不是佔用的空間。
- `u32` 和 `NonZeroU32` 同樣 4 byte。後者保證不是 0，存在的目的之一就是讓 `Option<NonZeroU32>` 維持 4 byte。
- `&str` 的 16 byte 全部都有用途。能借，是因為指標欄位的 2⁶⁴ 種值裡有一個（0）永遠不會被合法的 reference 用到。空字串的指標也不是 0。
- `bool` 邏輯上只要 1 bit，但記憶體最小定址單位是 byte，所以佔 1 byte、空出 254 個值。`Option<bool>` 有三種狀態，一個 byte 放得下。
- `Option<&str>` 的 `None` 在機器層和 C 的 `NULL` 是同樣的 bits。差別在型別層：`&str` 保證不是空的，`Option<&str>` 不先處理 `None` 就編不過。「Rust 沒有 null」指的是沒有「任何 reference 都可能偷偷是空的」這件事，執行期的表示法和成本都沒變。

### `impl` 與函式不佔值的空間

**核心規則**：`enum Command { … }` 描述資料，`impl Command { … }` 放函式。`size_of` 只看前者。函式是程式碼，整個程式一份，住在 `.text`。

- `impl` 是命名上的歸屬，不是把函式塞進每個值裡。把整個 `impl` 區塊刪掉，`size_of::<Command>()` 不變。
- `Command::execute(cmd, bag)` 編譯後是「跳到某個固定位址」，位址編譯期就決定。值裡面沒有任何東西指向函式。
- Rust 的 struct / enum 沒有隱藏欄位，定義了什麼欄位就是什麼。（每個物件帶一個指向類別的隱藏指標，是 Java、Python 的做法。）
- 定義 tuple 形式的 variant 之後，`Command::Add` 這個名字同時可以當建構函式用（`fn(String, u32) -> Command`）。定義處是資料的形狀，使用處看起來像呼叫函式；那個函式一樣住在 `.text`。
- 函式本身當成值來看是 0 byte（實測 `size_of_val(&main)` 是 0）；函式指標 `fn()` 是 8 byte。

### stack frame：一次呼叫的位址範圍

**核心規則**：frame 不是一個物件，是 stack 區域裡「屬於這一次函式呼叫」的那一段位址範圍。大小是編譯期常數，寫死在函式開頭的指令裡；執行期變動的只有「現在疊了幾個」。

```
depth:                ; 概念示意，數字含返回位址等雜項
    sub  rsp, 160     ; stack pointer 往下移 160 → frame 出現
    …函式內容…
    add  rsp, 160     ; 移回來 → frame 消失
    ret               ; 跳回 frame 裡存的返回位址
```

| | 什麼時候決定 | 誰決定 |
|---|---|---|
| 一個函式的 frame 多大 | 編譯期 | rustc |
| 變數在 frame 裡的位置 | 編譯期 | rustc |
| 現在 stack 上有幾個 frame、是誰的 | 執行期 | 程式實際怎麼跑 |
| stack 總共用了多少 | 執行期 | 各 frame 大小的加總 |

frame 裡放的東西：參數、區域變數、沒有名字的暫時值、padding、返回位址、保存的暫存器。區域變數的總大小是主要來源，不是精確公式：實測一個只有一個 `u32` 區域變數、內含 `println!` 的函式，debug build 的 frame 是 160 byte，多出來的大多是 `println!` 背後的暫時值。最佳化之後也可能更小（變數留在暫存器、位置共用、整個函式被 inline）。

什麼會產生新的 frame：

| 寫法 | 新 frame |
|---|---|
| 呼叫函式、方法、closure | 會（沒被 inline 的話） |
| `{ … }` 區塊、`if` / `match` 的分支 | 不會 |
| `for` / `while` / `loop` 的每一圈 | 不會 |

**關鍵細節**

- 邊界是每一次**呼叫**，不是每個函式**定義**。一個函式只定義一次，遞迴四層就同時有四個它的 frame；從沒被呼叫的函式一個都沒有。
- scope 和 frame 是兩回事：scope（每個 `{}`）管變數能不能用、何時 `Drop`，是編譯期規則；frame（每次呼叫）管變數的 byte 放在哪，是執行期的記憶體。內層區塊的變數離開 `}` 就不能用，但它那幾個 byte 要到函式返回才一起收回。
- 每次呼叫一個 frame 換來三件事：遞迴時每層的區域變數互不覆蓋、多條 thread 能同時跑同一個函式、函式不用知道誰呼叫它（返回位址在自己的 frame 裡）。
- 配置是「暫存器減一個數字」，回收是「加回來」，各一條指令，裡面有幾個變數都一次收完。擁有 heap 資源的變數，rustc 會在收回之前先插入它們的 `Drop`。
- 執行期沒有任何表格記錄 frame 的界線。界線靠 stack pointer 和每個 frame 裡的返回位址維持。
- 返回時 frame 的內容不會被清掉，只是 stack pointer 移回去；下一次呼叫的 frame 會直接蓋在上面。
- 編譯期知道每個 frame 多大，但不知道會疊多深，所以 stack overflow 只能在執行期發現。
- frame 的大小必須編譯期固定，這就是「大小不固定的東西進不來、只能放 heap」的另一種說法；`String` 留在 frame 裡的是固定 24 byte 的把手。
- 語言層的模型是「以值傳入的參數在被呼叫者的 frame 裡」。機器碼層，超過 16 byte 的值通常由呼叫端放在自己的 frame，再把位址交過去。不影響所有權的語意。

### frame 與 page：不同層

**核心規則**：frame 是程式自己在 stack 區域裡劃的位址範圍；page 是 kernel 和 MMU 把虛擬位址對應到實體記憶體的單位。兩者的邊界互不對齊。

| | stack frame | page |
|---|---|---|
| 屬於哪層 | 程式（rustc 產生的機器碼） | kernel 與 CPU 的 MMU |
| 是什麼 | 一次函式呼叫用的位址範圍 | 虛擬位址對應實體記憶體的最小單位 |
| 大小 | 每個函式不同，編譯期決定 | 固定，4 KB（Apple Silicon 是 16 KB） |
| 誰劃分 | 函式開頭的 `sub rsp, N` | kernel 設定的 page table |
| 誰知道它 | 只有程式自己 | 只有 kernel 和 CPU |

```
程式層     frame        「這 160 byte 是 depth(1) 的」          stack pointer 劃分
kernel 層  stack 區域    「這 8 MB 虛擬位址是這條 thread 的」    thread 建立時保留
           page         「這 4 KB 對應到哪塊實體記憶體」         page table
硬體層     實體記憶體    真正的 RAM
```

- 一個 4 KB 的 page 放得下很多個小 frame；宣告了大陣列的函式，單一 frame 也可以橫跨好幾個 page。
- stack 區域的 page 是第一次碰到才給實體記憶體（page fault，見第 5 節）。
- stack 區域最底下有一個不可存取的 guard page。frame 疊到那裡，寫入時 MMU 發現不可存取 → 通知 kernel → kernel 送 signal → std 印出 `has overflowed its stack`。
- OS 資料裡的 **page frame** 指「一個 page 大小的實體記憶體」，和 stack frame 無關，只是剛好同名。

### 思維鏈 A：`size_of::<Command>()` 怎麼算（全部發生在編譯期）

```
步驟  誰      做什麼
1     rustc   只看 enum Command { … } 這個區塊，impl 區塊不看
2     rustc   算每個 variant 的欄位：Add 28、Remove 24、List / Quit / Clear 各 0
3     rustc   取最大的 28
4     rustc   對齊 = 欄位中最大的對齊 = 8（String 裡有指標）
5     rustc   28 補到 8 的倍數 → 32
6     rustc   需要 5 種 tag（1 byte）；補出來的 4 byte 放得下 → tag 不另外佔空間
7     rustc   size_of::<Command>() = 32，當成常數嵌進用到它的機器碼
8     rustc   Option<Command>：tag 只用了 5 個值，借第 6 個代表 None → 仍是 32
9     執行期  沒有任何人在「計算大小」；match 只是讀 tag 再跳轉
```

### 思維鏈 B：遞迴四層的 frame 生滅

```rust
fn depth(n: u32) {
    let local = n;
    println!("depth({n})  local 的位址 = {:p}", &local);
    if n < 3 { depth(n + 1); }
}
```

實測四個 `local` 的位址依序相差 `0xa0`（160），越來越小。

```
時間  thread / mode    跑誰的程式碼           發生什麼                         stack
t0    main / user      main                   call depth(0)                    [main]
t1    main / user      depth 開頭（rustc 埋） sub rsp,160；local=0 寫進 frame   [main][d0]
t2    main / user      depth                  call depth(1)，返回位址存進 stack [main][d0]
t3    main / user      depth 開頭             sub rsp,160；local=1              [main][d0][d1]
t4    main / user      depth                  同上兩次                          [main][d0][d1][d2][d3]
t5    main / user      depth 結尾（rustc 埋） d3：add rsp,160；ret              [main][d0][d1][d2]
t6    main / user      depth 結尾             d2、d1、d0 依序返回               [main]
```

- 全程同一條 thread、全程 user mode，kernel 沒有進場。唯一可能進 kernel 的是 stack 長到一個還沒碰過的 page 時的 page fault，以及 `println!` 的 write。
- `depth` 的機器碼只有一份；`local` 有四份，各在自己的 frame。
- 把 `3` 換成執行期才知道的數字，會疊幾層編譯期就無法得知。

### 陷阱清單

1. ❌ `impl` 裡寫了很多函式，所以這個型別比較大 → ✅ `size_of` 只看 `struct` / `enum` 定義裡的欄位 → 為什麼：函式是程式碼，整個程式一份放在 `.text`，不在每個值裡。
2. ❌ `Add(String, u32)` 是函式，enum 裡面都是函式 → ✅ 那是欄位宣告，括號裡是欄位的型別 → 為什麼：enum 的每個 variant 都只是一種 struct；`Command::Add(a, b)` 在使用處才是呼叫建構函式。
3. ❌ 函式的大小 = 參數 + 回傳值 → ✅ 函式有兩種大小：機器碼長度（看內容）與 frame 大小（看變數與暫時值） → 為什麼：參數只是 frame 裡的一部分；小的回傳值走暫存器，不佔 frame。
4. ❌ stack 上有東西在「表達這個函式」 → ✅ 函式本身只在 `.text`；stack 上的是某一次呼叫的 frame → 為什麼：程式碼和它工作時用的資料分開放。
5. ❌ 機器碼放在 `.rodata` → ✅ 機器碼在 `.text`，`.rodata` 放唯讀資料 → 為什麼：`.text` 的頁有執行權限，`.rodata` 沒有。
6. ❌ frame 的大小像 meta，實際多大執行期才知道 → ✅ 單一 frame 的大小是編譯期常數，寫死在函式開頭的指令裡；執行期才知道的是疊了幾個 → 為什麼：放在 stack 上的東西大小都必須編譯期已知，rustc 才算得出來。
7. ❌ frame 的邊界是 scope，或是每個函式一個 → ✅ 每一次函式呼叫一個 → 為什麼：`{}` 和迴圈都共用所在函式的 frame；遞迴時同一個函式同時有多個 frame。
8. ❌ frame 的大小 = 所有區域變數的總和 → ✅ 那是主要來源，還有暫時值、padding、返回位址、保存的暫存器；最佳化後也可能更小 → 為什麼：rustc 算的是「這個函式同時最多需要多少 stack」。
9. ❌ frame 是 page 裡的上下界 → ✅ frame 是 stack 區域裡的位址範圍，和 page 是不同層，邊界互不對齊 → 為什麼：frame 由 stack pointer 劃分，kernel 不知道；page 由 page table 劃分，程式不知道。
10. ❌ 值的大小、機器碼大小、frame 大小之間有固定的大小關係 → ✅ 只有「frame ≥ 直接放在它裡面的值」 → 為什麼：值也可以放在 heap，那時和任何 frame 都無關。
11. ❌ `&str` 的 16 byte 用滿了，沒有空間可以借給 `Option` → ✅ 借的是「不會出現的值」：指標欄位不可能是 0 → 為什麼：空位是「能表示的組合數」減「合法值的數量」，和有沒有多餘的 byte 無關。
12. ❌ `bool` 是 2 bit → ✅ 邏輯上 1 bit、實際佔 1 byte，空出 254 個值 → 為什麼：需要 2 bit 的是 `Option<bool>` 的三種狀態，一個 byte 放得下。
13. ❌ `Option` 有一個固定的大小 → ✅ `Option<T>` 的大小看 `T`：`T` 有空位就和 `T` 一樣大，沒有就加 tag 再補對齊 → 為什麼：`Option` 只是一個普通的 enum。
14. ❌ `List` 沒有欄位，所以 `Command::List` 這個值是 0 byte → ✅ 變數的型別是 `Command`，一律 32 byte → 為什麼：同一個型別的值必須一樣大。
15. ❌ `Vec` 在 stack 上的部分分別指向 heap 上每一個元素 → ✅ 一個指標指向一塊連續空間，元素一個挨一個 → 為什麼：`Vec` 靠「每個元素一樣大」用乘法算位置。
16. ❌「rustc 4 byte 一組」 → ✅ 正式名稱是 alignment：型別的對齊取欄位中最大的，大小補到對齊的整數倍 → 為什麼：對齊是 8 的型別就不是 4 byte 一組了。
17. ❌ `Option<Command>` 是 32，因為對齊有多餘空間可以用 → ✅ 分兩步：`Command` 的 tag 放在對齊補出來的 byte 裡，所以 `Command` 是 32；`Option` 再借 tag 沒用到的值代表 `None`，所以沒有變大 → 為什麼：`Option` 這一層沒有用到任何新的 byte，它和 `Command` 共用同一個 tag。
18. ❌ 能借的只有指標（位址 0），`Command` 不是指標所以沒得借 → ✅ 任何欄位只要有不會出現的值都能借：`bool`、`char`、enum 的 tag、`NonZero*` → 為什麼：判斷標準是「有沒有永遠不會出現的 bit 組合」，和是不是指標無關。

---

## 7. Rc / Arc / Send / Sync / 'static

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

- `static` 那一列放在哪個區段、能不能改、跟 `const` 差在哪，在第 5 節「`const` 與 `static`」
- `'` 開頭是 lifetime 名稱，`'static` 是唯一內建的固定名稱
- `tokio::spawn` 要求 `'static`：task 由 runtime 擁有，可能活得比呼叫函式久，不能借用區域變數 → 需要 `move` + `Arc::clone`
- 不需要 Send/'static 的情況：`.await`、`join!`（子 Future 屬於當前 task，不會單獨跨執行緒、不會活更久）；`thread::scope` 允許借用（結構化並發）

---

## 8. Closure 與 move

- `|x| x + 1`：`|...|` 是參數列表，不是 or；`||` 代表無參數
- 可捕獲外部變數（`fn` 不行）；編譯器自動選最溫和的捕獲：只讀 → `&`、修改 → `&mut`、消耗 → 取走所有權
- `move` 強制全部以所有權方式捕獲；`thread::spawn` / `tokio::spawn` 需要它（E0373）
- `move` 只決定「捕獲怎麼存」，不決定能呼叫幾次；`move` closure 仍可是 `Fn`
- `Fn`（只讀、多次）/ `FnMut`（修改、多次）/ `FnOnce`（消耗、一次）
- 本質是編譯器生成的匿名 struct，捕獲變數是欄位；不捕獲則 0 bytes，零成本
- Go/JS closure 一律參考捕獲 + GC 兜底；Rust 沒 GC 所以要明確選借用或擁有

---

## 9. async / Future / Waker

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

## 10. Tokio runtime 結構

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

## 11. 作業系統層

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

> 從記憶體這一側看同一條線（哪些記憶體動作進 kernel、哪些不進，page fault 算哪一種）在第 5 節「kernel 什麼時候在場」。

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

## 12. 其他概念

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

## 13. 誤解修正記錄（一路上被校正過的點）

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

copy / move / clone / borrow 那條線被校正的 17 點在第 4 節的陷阱清單，保留「為什麼」一欄，所以不併進這張兩欄表。process 記憶體區域與 kernel 那條線被校正的 10 點在第 5 節的陷阱清單，同樣不併進來。型別大小、enum 與 stack frame 那條線被校正的 18 點在第 6 節的陷阱清單。
