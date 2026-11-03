# Note 4 — 借用、獨佔與壽命：從 `&` 挖到 ELF 區段

> 這份筆記記錄的是「`&` 到底是什麼」這條線：從 expression-based 與區塊回傳起步，穿過借用規則、borrow checker 所在的層、獨佔／原子／互斥三個詞的分辨、閉包與 `thread::spawn`，到 lifetime 與 `&'static str`，最後落到 `static` / `const` 與 ELF 區段。
> 核心收穫不是語法，而是**每一條規則在哪一層生效（型別系統 / 語言 / std / OS / 硬體 / binary）、到了執行期還剩下什麼**——borrow checker 編譯完就消失，獨佔讓「原子與否」這個問題消失，`'static` 是壽命不是位置。
>
> 記錄日期：2026-11-03｜當時定位：🥚 小白（仍未動手寫程式），這條線概念上走完 🐣 的 borrowing、🦀 的 lifetime，並碰到 🦞 的 Mutex / Atomic 與 🐊 的記憶體佈局 / ELF。

---

## 0. 一句話版本

`&` 是名片不是房子；borrow checker 在編譯期檢查「多讀 XOR 單寫」的區間重疊，執行期不存在；獨佔把觀察者趕走所以不需要原子，原子只用在多執行緒搶獨佔的那一瞬間；lifetime 是參考的標籤不是變數的壽命，`'static` 是「活到程式結束」不是「放在 static 區」。

---

## 1. 分層模型（骨架）

```
型別系統層    'a / 'static、&T / &mut T、T: 'static      編譯期標籤，borrow checker 檢查完擦掉
語言層        let / mut / static / const / move / 閉包      決定所有權、可變性、進哪個 section
std 層        Mutex、Atomic*、Arc、thread::spawn           把獨佔/原子/共享包成型別
OS 層         futex wait queue、clone(2)、loader mmap       執行期的睡眠喚醒、thread 建立、區段載入
硬體層        lock cmpxchg / lock xadd、對齊 word 存取       原子性真正的來源
binary 層     ELF / Mach-O：.text .rodata .data .bss        字面值住在 .rodata
```

Rust 關鍵字五類：

```
A. 綁定與可變性   let  mut  const  static  shadowing
B. 控制流         if  match  loop  while  for  break  continue  return  if let
C. 型別定義       struct  enum  type  impl  trait  dyn  Self
D. 所有權與借用   &  &mut  move  ref  'a
E. 組織與可見性   mod  pub  use  crate  self  super  as
```
A/B/E 與 Go 近乎一對一；C 是 Go 沒有的表達力（enum 帶資料、trait 明確 impl）；D 是 Go 用 GC 迴避掉的問題。

---

## 2. Expression-based 與區塊回傳

核心規則：**區塊最後一個不加分號的 expression 就是區塊的值**；函式本體是區塊，所以最後一行 `sign` 等價 `return sign;`。

關鍵細節：
- `x + 1` 是 expression 有值；`x + 1;` 是 statement，值為 `()`。函式最後一行多加分號 → `expected &str, found ()`
- `if` / `match` / `loop { break val }` 都是 expression，兩邊型別必須一致
- `match` 窮舉是編譯期檢查；加 enum 變體後所有沒處理的 `match` 都會編譯失敗——除非用了 `_`
- `v.push(4)` 的 `push` 簽名是 `fn push(&mut self, T)`，編譯器自動做 `(&mut v).push(4)`（auto-ref）；方法呼叫會依簽名產生 `&self` 或 `&mut self`

---

## 3. `&` 的本質：名片，不是房子

核心規則：**`&v` 複製的是 v 的位址（8 bytes），不是 v 的資料**。Rust 全部 call by value，只是這裡被複製的值是一個位址。

```
stack（main frame）                        heap
v  [ptr│len│cap] ──────────────────────► [1, 2, 3, (4)]   ← 全程式只有這一份
a  [v 的位址] ──┐
b  [v 的位址] ──┼──► 指向 stack 上的 v，不是直接指 heap
c  [v 的位址] ──┘
```

關鍵細節：
- a、b **作為變數**是 stack 上兩個不同格子（`&a != &b`），**裝的值**相同（`a == b`，都是 `&v`）
- `c.push(4)` 的路徑：透過 c 找到 v → 透過 v.ptr 找到 heap → 寫入 → **回頭改 v.len**。所以 v 這格有異動（len 3→4），a、b 本身沒變，但透過它們看到的 v 已是新的
- `v.clone()` 才會配置第二塊 heap；`&` 從不複製被借的東西——借用的全部目的就是不複製、不轉移所有權
- 兩種 `mut` 不同：`let mut a = &v` 管 a 能否重新指向別人；`let a = &mut v` 管能否透過 a 改資料

| | C `*p` / Go `*T` | Rust `&T` / `&mut T` |
|---|---|---|
| 執行期 | 一個位址 | 一個位址，完全一樣 |
| 可 null / dangling | 可 | 不可（編譯期保證） |
| 同一資料的可變指標數 | 任意 | 最多一個，且此時無唯讀指標 |
| 誰保證 | 自己 / GC（Go 只保證不 dangling） | 編譯器，編譯完就沒了 |

驗證程式：
```rust
let mut v = vec![1, 2, 3];
let a = &v; let b = &v;
println!("{:p} {:p}", &a, &b);            // 不同：兩個 stack 格子
println!("{:p} {:p} {:p}", a, b, &v);     // 三個相同：都是 v 的位址
println!("{:p} {:p}", a.as_ptr(), b.as_ptr()); // 相同：同一塊 heap
let d = v.clone();
println!("{:p}", d.as_ptr());             // 不同：這才是第二棟房子
```

---

## 4. 借用規則：多讀 XOR 單寫

核心規則：**對任何值，任一時刻要嘛多個 `&`，要嘛一個 `&mut`（或擁有者自己改），不能同時。借用從建立持續到最後一次使用（NLL）；在這區間內擁有者的權限被降級。**

| 活著的借用 | 擁有者在這期間 | 借用者可以 |
|---|---|---|
| 一個或多個 `&` | 只能讀，不能改、不能 move | 讀 |
| 一個 `&mut` | **完全不能碰，連讀都不行** | 讀和改 |

關鍵細節：
- 規則對**任何值**成立，不限 heap：`let mut x = 5; let r = &x; x = 6; println!("{r}")` 一樣擋
- 借用的結束點只由「最後一次使用在哪一行」決定。異動不會「結束」借用；正確敘述是：**異動那一行，所有指向該值的 `&` 必須已經結束**
- 「之前」或「之後」都可以，判斷只問一件事：`&mut` 活著的那幾行，有沒有別的 `&` 也活著
- `v.push()` 的隱式 `&mut` 區間只有一行，比 `let c = &mut v` 存起來不容易撞——借用越短命衝突越少
- 擋的理由不是「這行會壞」，是「這個形狀會壞」：`let a = &v[0]; v.push(4)` 可能 realloc → a 指向已釋放記憶體。borrow checker 不分析 push 會不會剛好 realloc，只認形狀
- 官方名稱：`&T` = **shared** reference，`&mut T` = **exclusive** reference。`mut` 真正表達的是獨佔，不是可變；這就是為什麼 `&mut` 活著時擁有者連讀都不行

---

## 5. borrow checker 在哪一層

核心規則：**borrow checker 是編譯期的「區間重疊檢查」，不是執行期的「讀寫排程器」。**

```
編譯期   rustc 檢查借用區間在原始碼行數上有沒有重疊   ← borrow checker 在這
執行期   CPU 跑指令，才有時間、原子、交錯、同步       ← borrow checker 不存在
```

關鍵細節：
- 編譯出的機器碼和 C 一模一樣：沒有鎖、沒有等待、沒有檢查指令，零成本
- 單執行緒下程式碼本來一行一行跑，「交錯」不可能發生；規則保護的是「**重疊區間內透過兩個不同名字看同一份資料**」這個邏輯錯誤
- 不存在任何執行期的中央管理者在等、在切換、在保證原子

---

## 6. 三個詞：獨佔、原子、互斥

核心規則：**三者詞性不同，不能做集合判斷，只能做依賴判斷。**

| 詞 | 定義 | 形容的對象 |
|---|---|---|
| 原子 atomicity | 一個**操作**要嘛整個做完、要嘛沒做，看不到中間狀態 | 動詞（push、write、fetch_add） |
| 互斥 mutual exclusion | 同一時刻只有**一個存取者** | 名詞（把手、路徑） |
| 借用規則 | 多個讀者 **或** 一個寫者 | 名詞（把手、路徑） |

關鍵細節：
- 操作是動詞，需要時間，有開始/中間/結束；存取路徑是名詞，不佔時間，只有存在/不存在。問「互斥 ⊂ 原子？」像問「紅色 ⊂ 跑步？」
- **獨佔不是讓操作變原子，是讓「原子與否」這個問題消失**：`&mut` 下 `push` 仍是兩步（寫元素、改 len），但沒有觀察者，中間狀態無法被觀察
- 「多讀」不是反例：讀不製造中間狀態，危險只有「有人寫 + 有人看」這一種組合。純互斥（Go `sync.Mutex`）連讀+讀也擋，所以借用規則比互斥寬，對應 `RWMutex`
- 共同祖先叫 **linearizability**：不管實際多少東西同時跑，觀察到的結果等價於一個接一個完整發生。原子（動詞不可分割）、獨佔（名詞只有一條路）、borrow checker（編譯期禁止危險形狀）是三種手段
- 「物理上一個」是錯的，「觀察上一個」才對：`fetch_add` 十條 thread 物理上同時進，結果仍正確

依賴鏈（疊磚，不是並行）：
```
硬體層    原子指令（CAS、xadd）          原子性真正來源，只對一個 word 有效
std 層    Mutex（用 CAS 搶 + 睡眠喚醒）   把「一個 word 的原子」放大成「一段程式碼的獨佔」
語言層    &mut 獨佔規則                  把獨佔編進型別，編譯期檢查，執行期零成本
```
互斥是「讓非原子操作看起來像原子」的手段；互斥本身的實作需要一條真正原子的硬體指令。兩個方向都不是子集。

反義詞：

| 軸 | 正 | 反 | 反面的現象 |
|---|---|---|---|
| 操作 | 原子 | 可分割（divisible） | torn read/write：看到 `{x:10, y:0}` 這種從未存在的值 |
| 存取 | 獨佔（exclusive） | 共享（shared / aliasing） | 同一資料多個名字，編譯器不敢把值快取進暫存器 |

四格表與工具：

```
                 操作可分割          操作原子
存取獨佔         安全（沒人看中間）   安全
存取共享         危險 ← data race    安全（lock-free）

Mutex      把共享變獨佔（往上移）
Atomic*    把可分割變原子（往右移）
&mut 規則  編譯期禁止站在左下角
& 多讀     讀不製造中間狀態，共享+讀不在危險區
```
決策順序：先看存取是否共享 → 共享才需要考慮 → 操作能否做成原子（單 word read-modify-write 可以）→ 可以用 atomic，不行才上鎖。

四格範例（精簡）：
```rust
// 格 1  獨佔 × 可分割：&mut 唯一路徑，兩步之間無觀察者
let c = &mut v; c.push(4);

// 格 2  共享 × 只讀：Arc 只是讓 Vec 活到所有 thread 結束
let v = Arc::new(vec![1,2,3]);
thread::spawn({ let v = Arc::clone(&v); move || println!("{v:?}") });

// 格 3  共享 × 原子：沒有鎖、沒人等，每個 fetch_add 是一條 lock xadd
let n = Arc::new(AtomicU64::new(0));
n.fetch_add(1, Ordering::Relaxed);

// 格 4  共享 × 可分割 → Mutex 把共享「暫時變獨佔」
let v = Arc::new(Mutex::new(vec![]));
let mut g = v.lock().unwrap();   // 搶獨佔：一條 CAS；搶不到就睡
g.push(i);                       // g 解參考是 &mut Vec，回到格 1 的世界
                                 // g drop → 解鎖 → 喚醒下一個

// 左下角沒有工具 → E0499
let a = &mut v; let b = &mut v;
```
格 4 的 `g.push(i)` 和格 1 的 `c.push(4)` 是同一行、同一型別 `&mut Vec`。Mutex 只是在執行期製造出一個 `&mut`，之後的安全性完全交給格 1 的規則。

---

## 7. 閉包與 `thread::spawn`

核心規則：**閉包是編譯器生成的匿名 struct，捕獲的變數就是它的欄位。沒有任何執行期計數。**

```rust
let f = move || println!("{i} {v:?}");
// 等價於
struct Closure { i: i32, v: Arc<Vec<i32>> }
impl FnOnce<()> for Closure { fn call_once(self) { ... } }
let f = Closure { i, v };
```

| 寫法 | 欄位型別 | 意思 |
|---|---|---|
| `\|\| use(&v)` | `&Vec` | struct 裡存借用，借用規則照常在編譯期檢查 |
| `move \|\| use(v)` | `Vec` / `Arc<Vec>` | 值搬進 struct，原變數不能再用（Copy 型別則複製） |

關鍵細節：
- `Arc::clone(&v)` 那一行才是計數，但那是 `Arc` 型別自己的事，和閉包機制無關，而且是明確寫出來的
- `thread::spawn` 要求閉包 `F: 'static`：新 thread 可能比 main 的 stack frame 活得久，閉包不能持有對外部 stack 的借用，所以只能 move
- `std::thread::spawn` 沒有 pool：每次 = 一次 `clone(2)` syscall = 新 OS thread = 新 tid，跑完就結束。tokio worker pool 建在這上面
- `Arc` 解決的是「資料活到所有 thread 結束」，和獨佔/原子無關

---

## 8. 壽命 lifetime

核心規則：**lifetime 是參考的標籤，回答「這個參考指向的資料保證活到哪」。唯一的規則：參考的壽命 ≤ 被指資料的存活區間。只存在於編譯期。**

```rust
let r;
{ let x = 5; r = &x; }   // x 死
println!("{r}");          // E0597 `x` does not live long enough
```

### `'a` vs `'static`：泛型 vs 具體

| | 型別軸 | 壽命軸 |
|---|---|---|
| 泛型參數（呼叫時決定） | `T`、`U` | `'a`、`'b` |
| 具體值（寫死） | `i32`、`str` | `'static`（唯一有名字的具體壽命） |
| 宣告位置 | `fn f<T>` | `fn f<'a>` |

- `&'a T` 裡有**兩個**泛型參數：`'a` 壽命、`T` 型別，互不相干。`&'a str` 只有一個泛型參數
- `'a` 名字隨便取（`'b`、`'input` 都行），只有 `'static` 和 `'_` 保留。**同名才是重點**：同名 = 綁在同一區間
- `fn longer<'a>(x: &'a str, y: &'a str) -> &'a str`：呼叫時 `'a` 被填成 x、y 壽命的**交集**。即使函式實際回傳 `&s1`，編譯器也只看簽名，s2 死後 result 就不能用
- `fn pick<'a, 'b>(x: &'a str, y: &'b str) -> &'a str`：兩個獨立壽命，回傳只綁 x，s2 死不影響 result。多個名字的存在理由就是**告訴編譯器哪些參考之間有關係**
- `'static` = 從程式開始到結束。能標的資料只有兩種：編進 binary 的（字面值、`static`），或刻意永不釋放的（`Box::leak`）。定義是**壽命**，不是**存放位置**

### 語法位置與省略規則

- `'a` 永遠緊跟 `&`：`&'a T`、`&'a mut T`。`Vec<'a String>` 不合法（String 不是參考，沒壽命可標）；`Vec<&'a String>` 是另一種資料結構
- `'T` 語法上是「一個叫 T 的壽命」，和型別 `T` 無關；`'static str` 沒有 `&` 就沒有主詞
- 省略規則（elision）：

```rust
fn first(v: &Vec<i32>) -> &i32          // ✅ 一個參考輸入 → 輸出自動綁它
fn longer(x: &str, y: &str) -> &str     // ❌ E0106：兩個輸入，綁誰？
fn name() -> &str                       // ❌ E0106：沒有輸入 → 必須 'static
fn get<'a>(v: &'a Vec<String>, key: &str) -> &'a String   // 回傳指向 v 的元素，綁 v 不綁 key
```
`&T` 是省略了 `'a` 的寫法，不是另一種東西。要手寫的時機只有一個：編譯器從簽名看不出回傳的借用來自哪個參數。

### `T: 'static` vs `&'static T`

| 寫法 | 意思 |
|---|---|
| `&'static T` | 一個**參考**，指向活到程式結束的資料 |
| `T: 'static` | **型別約束**：T 裡面不含任何非 `'static` 的參考 |

`String`、`Vec<i32>`、`Arc<Mutex<..>>` 全部滿足 `T: 'static`——擁有型別，沒有借用，「想活多久就活多久」。`&'a Vec` 不滿足。讀作「T 沒有壽命上的牽絆」，不是「T 住在 static 區」。

### 編譯器怎麼處理泛型：單態化 vs 擦除

```
t0  rustc 讀到 fn spawn<F: FnOnce() + Send + 'static>   只檢查本體用了 F 承諾的能力；不產生程式碼
t1  讀到呼叫 spawn(closure_A)                           檢查 closure_A 滿足三個 bound（含 borrow checker）
t2                                                      生成 spawn::<closure_A>，所有大小已知
t3  讀到呼叫 spawn(closure_B)                           生成 spawn::<closure_B>
t4  codegen                                             兩份獨立機器碼
```

```
型別參數 T     → 單態化（monomorphization）→ 影響機器碼（大小、佈局、呼叫哪個函式）
壽命參數 'a    → 檢查後擦除 → 不影響機器碼；&'static T 和 &'a T 編出來一模一樣
```
零成本抽象的機制：泛型執行期不存在，每個呼叫直接呼叫具體版本。代價是 binary 變大、編譯變慢。Go 泛型走 gcshape + dictionary，執行期帶型別資訊，有少量成本。

---

## 9. `&'static str` 三部分拆解

| 部分 | 回答 | 內容 |
|---|---|---|
| `&` | 怎麼拿 | 借用；`str` 是 unsized，必須透過指標拿 |
| `'static` | 拿多久 | 指向的資料活到程式結束 |
| `str` | 拿什麼 | 型別（不是變數名）：一段長度不定的 UTF-8 bytes |

核心規則：**`str` 是 unsized 不是「每個值大小都未知」，是「`str` 這個型別沒有統一大小」**。`"hi"` 2 bytes、`"hello"` 5 bytes 都是 `str`，所以不能 `let s: str`，不能按值回傳（stack frame 要固定大小）。

大小存在 fat pointer 裡：
```
&str  =  [ ptr: 8 bytes │ len: 8 bytes ]  = 16 bytes，大小固定
&i32  =  [ ptr: 8 bytes ]                 = 8 bytes，thin pointer
```

| 回傳型別 | 把手 | 資料在哪 | 成本 |
|---|---|---|---|
| `&'static str` | ptr + len | `.rodata`，編進 binary | 零 |
| `&'a str` | ptr + len | 某個參數指向的地方 | 零，壽命綁參數 |
| `String` | ptr + len + cap | heap，函式自己配置 | 一次 malloc + memcpy |

字面值回傳 `&'static str` 不是「必須借用」，是「借用最便宜且正確」；回傳 unsized 是編譯錯誤 E0277，不是執行期炸。

---

## 10. `static` / `const` / `'static` / section / ELF

核心規則：**`'static` 是型別系統的標籤；`static` 是語言的儲存類別；`.rodata` 是 binary 的區段；三者分屬不同層。**

| Rust | 是什麼 | 放哪 |
|---|---|---|
| `static X: u32 = 5;` | 全程式唯一變數，有固定位址 | `.rodata`（不可變）/ `.data`（可變） |
| `const X: u32 = 5;` | 編譯期常數，**沒有位址** | 不佔 section；每個使用處 inline 成立即數 |
| `'static` | 壽命標籤 | 哪裡都不放；檢查完擦掉 |

關鍵細節：
- C 的預處理（cpp）是純文字替換（`#define`、`#include`），和 static 儲存無關；static 資源進 section 是編譯器 + linker 的事
- Rust 沒有預處理器；`macro_rules!`、`println!` 在 AST 層展開，是編譯本身
- `.rodata` 之所以重要：字面值的 bytes 被放進去，loader 啟動時映射進位址空間，process 結束才解除——**壽命物理上就是整個程式**，所以型別系統敢給 `'static`；頁是唯讀的，所以永遠是 `&` 不是 `&mut`

### ELF

Executable and Linkable Format：Linux 上可執行檔、`.so`、`.o`、core dump 共用的**檔案格式**。macOS 是 Mach-O（用 `otool -l` / `size`），Windows 是 PE；結構概念相同。

```
┌─────────────────────┐
│ ELF header          │  魔數 7F 45 4C 46、架構、入口點、兩張表在哪
├─────────────────────┤
│ Program headers     │  給 loader：哪幾塊 mmap 到哪、什麼權限
├─────────────────────┤
│ .text               │  機器碼                      r-x
│ .rodata             │  字面值、不可變 static、跳轉表  r--
│ .data               │  有初值的可變全域             rw-
│ .bss                │  零初值可變全域，檔案裡只記大小  rw-
│ .symtab / .strtab   │  符號表（debugger、linker 用）
│ .rela.*             │  重定位表
│ .dynamic / .dynsym  │  動態連結資訊：依賴哪些 .so
├─────────────────────┤
│ Section headers     │  給 linker：每個 section 的名字、偏移、大小
└─────────────────────┘
```

| 誰讀 | 看哪張表 | 單位 | 關心什麼 |
|---|---|---|---|
| linker（ld） | Section headers | section | 合併同名 section、解析符號 |
| loader（kernel + ld.so） | Program headers | segment | mmap 哪幾塊、設 r/w/x |

| 檔案 | 有什麼 | 缺什麼 |
|---|---|---|
| `.o` | section + 符號 + 重定位表 | 沒有 program headers，位址未定 |
| 可執行檔 | section + program headers + 入口點 | 重定位大多已做完 |
| `.so` | 同可執行檔但位址無關（PIC） | 沒有入口點，等 `dlopen` 或被依賴 |

`.so` 是檔案，`.rodata` 是檔案裡的區段；`.so` 裡面有 `.rodata`。`cargo build`：每個 crate → `.o` / `.rlib` → linker 合併 → 可執行檔。

---

## 11. 思維鏈

### A. 借用區間檢查（編譯期，看行數不看時間）

```
行   程式碼                     a 的區間      c 的區間      判定
1    let a = &v;                ┐
2    println!("{a:?}");         ┘ 結束於此                  
3    let c = &mut v;                          ┐             a 已結束 → OK
4    c.push(4);                               ┘ 結束於此
5    println!("{v:?}");                                     無活著的借用 → OK

若在行 5 之後加 println!("{a:?}")：
1–6  a 的區間延伸到行 6          ┐
3–4  c 的區間                    │  ┐         行 3–4 兩者重疊 → E0502
                                 ┘  ┘
```

### B. Mutex 四 thread（執行期，原子只在搶鎖那一瞬）

```
t    thread   mode     跑誰的程式碼            發生什麼
t0   A        user     Mutex::lock()           CAS 指令（lock cmpxchg）── 唯一的硬體原子操作
t1   A        user     拿到 MutexGuard         A 持有 &mut Vec，獨佔成立
t2   B        user→k   Mutex::lock() → futex   CAS 失敗 → syscall → 進 kernel wait queue 睡
t3   A        user     g.push(i)               兩步、非原子；B 睡著，無觀察者
t4   A        user→k   guard drop → futex_wake 解鎖 + 喚醒 B
t5   B        k→user   拿到 MutexGuard         B 持有 &mut Vec，獨佔成立
```
原子性只在 t0；t1–t4 回到「獨佔 → 不需要原子」的世界。Mutex = 用一次原子操作換到一段執行期獨佔，再包成 `&mut` 交回借用規則。

### C. 閉包 + Arc 生命週期（沒有閉包層的計數）

```
t0   main     Arc::clone(&v)         refcount 1→2，第二個把手（Arc 的事，明確寫出）
t1   main     move || {...}          把手 2 搬進閉包 struct 的欄位
t2   main     thread::spawn(f)       clone(2) 建新 thread；struct 整個搬到新 thread 的 stack
t3   worker   閉包跑完               struct drop → 欄位 drop → refcount 2→1
t4   main     v drop                 refcount 1→0 → Vec 釋放
```

---

## 12. 陷阱清單

1. ❌ 「v 指向 c」 → ✅ 箭頭永遠是借用者 → 擁有者；a、b、c 都指向 v，v 不指向任何人 → 為什麼：`&v` 是從 v 取位址，v 是擁有者，從頭到尾是同一個變數。

2. ❌ a、b、c 各自對應一個 heap vector → ✅ 只有一份 heap 資料；a、b、c 裝的都是 v 的位址，甚至不是直接指 heap 而是指 stack 上的 v → 為什麼：`&` 從不複製被借的東西，複製要明確寫 `clone()`。

3. ❌ 「這是 call by reference」 → ✅ Rust 全部 call by value，被複製的值是一個位址 → 為什麼：`let a = &v` 和 `g(&v)` 都是把 8 bytes 位址複製進去。

4. ❌ 「改 c 會不會改 a、b？」 → ✅ `c.push` 改的是 heap 資料和 v.len；a、b 的值（位址）沒變，透過它們看到的資料變了 → 為什麼：沒有第二份資料，所以沒有「同步」問題。

5. ❌ 「a、b 不是 mut 所以不變」混了兩種 mut → ✅ `let mut a` 管 a 能否重指；`&mut v` 管能否透過它改資料 → 為什麼：綁定可變性與參考可變性是兩個獨立軸。

6. ❌ 借用規則只針對 heap → ✅ 對任何值成立，`let r = &x; x = 6; use(r)` 一樣擋 → 為什麼：借的是擁有者那個值，不是 heap。

7. ❌ 「借用期間不可異動」只講一半 → ✅ `&` 活著：擁有者只能讀；`&mut` 活著：擁有者連讀都不行 → 為什麼：`&mut` 的本義是獨佔，多一條路徑（即使只讀）就不獨佔。

8. ❌ 「異動表示 borrow life end」 → ✅ 借用結束只由最後一次使用決定；異動那行所有 `&` 必須已結束，否則拒絕那行異動 → 為什麼：編譯器不是把 a 殺掉，是拒絕非法的那次寫。

9. ❌ borrow checker 保證「讀寫原子、不交錯、同步完成後才換」 → ✅ 它是編譯期區間重疊檢查，執行期不存在，機器碼和 C 一樣 → 為什麼：沒有任何執行期的中央管理者在等或切換（反覆出現的「幻想中央管理者」習慣）。

10. ❌ 「滿足獨佔必然原子」 → ✅ 獨佔下 `push` 仍是非原子兩步，只是沒有觀察者，原子與否無定義 → 為什麼：原子是操作的屬性，獨佔是路徑的屬性。

11. ❌ 「原子性 = 單位時間只能一個存取者」 → ✅ 那是互斥的定義；原子性是「操作不能被看到做一半」 → 為什麼：多讀不是反例，讀不製造中間狀態。

12. ❌ 「互斥 ∈ 原子性」 → ✅ 詞性不同，無集合關係；互斥是讓非原子操作看起來像原子的手段，而互斥實作需要一條原子指令 → 為什麼：`fetch_add` 原子但無互斥（lock-free）；鎖內 `push` 互斥但非原子。

13. ❌ 「期間內只有一個物理量」 → ✅ 是「觀察上像只有一個」（linearizability） → 為什麼：十條 thread 物理上同時 `fetch_add`，結果仍正確。

14. ❌ 「疊磚 = 並行獨立的概念」 → ✅ 疊磚是依賴：上層用下層蓋，拿掉下層上層塌 → 為什麼：Mutex 用 CAS，`&mut` 規則把 Mutex 給的獨佔編進型別。

15. ❌ 閉包引用外部變數靠執行期計數，結束時 release → ✅ 閉包是匿名 struct，捕獲變數是欄位；計數是 `Arc::clone` 明確做的，和閉包無關 → 為什麼：Rust 沒有 GC，捕獲方式（借用或 move）在編譯期決定。

16. ❌ lifetime 是「某個函式的 stack frame」「在呼叫者 f() 裡」 → ✅ lifetime 標的是被指資料活多久，`f() -> &'static str { b() }` 回傳後 main 仍能用 → 為什麼：frame 生生滅滅，`.rodata` 的資料一直在。

17. ❌ lifetime 是「變數的生命週期」 → ✅ 變數生死由 ownership/scope/Drop 管；lifetime 是參考的標籤，`fn f<'a>` 簽名裡沒有任何變數 → 為什麼：lifetime 是不等式「參考壽命 ≤ 被指資料存活區間」的標記工具。

18. ❌ `'a` 和 `T` 是同一個東西、`&'a T` 等價 `&T` → ✅ 同一個 `<>` 裡兩個互不相干的參數；`&T` 只是省略規則能推時的簡寫 → 為什麼：`'a` 之於 `'static`，正如 `T` 之於 `i32`。

19. ❌ `&'T str`、`'static str`、`'str` → ✅ `'` 後是壽命名字（慣例小寫）；壽命必須黏在 `&` 後；`str` 是型別不是變數名 → 為什麼：壽命和型別在不同命名空間，沒有 `&` 就沒有主詞。

20. ❌ `&Vec<'a String>` → ✅ `&'a Vec<String>` → 為什麼：`'a` 標參考，`String` 是擁有型別沒壽命可標；`Vec<&'a String>` 是另一種結構。

21. ❌ 「回傳動態大小不借用記憶體會炸」 → ✅ 是編譯錯誤 E0277；也可回傳 `String` 只是多一次配置 → 為什麼：unsized 的意思是型別沒有統一大小，`"hi"` 本身大小編譯期完全確定，記在 fat pointer 的 len 裡。

22. ❌ `'static` = global / 放在 static 區 / 預處理 block → ✅ `'static` 是「活到程式結束」的壽命標籤；`Box::leak` 的 heap 資料也是 `'static` → 為什麼：定義的是壽命不是位置；型別系統標籤、語言儲存類別、binary 區段是三層。

23. ❌ C 的 static 資源放在預處理 block → ✅ 預處理只做文字替換；static 資源進 `.data/.rodata/.bss` section，是編譯器 + linker 的事 → 為什麼：「預處理 block」不存在，想的是 section。

24. ❌ `.so` 和 `.rodata` 是同類概念 → ✅ `.so` 是檔案（ELF 的一種用法），`.rodata` 是任何 ELF 檔內部的區段 → 為什麼：ELF = header + sections + 兩張表；linker 讀 section headers，loader 讀 program headers。

25. ❌ 格 4 的「操作可分割」讀成「可以切割原子性」 → ✅ 是 `push` 本身非原子（兩步）；Mutex 讓同一時刻只有一條 thread 持 `&mut`，兩步之間無觀察者 → 為什麼：和格 1 同一個安全理由，獨佔從編譯期保證換成執行期搶鎖。
