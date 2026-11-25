# q05：RPN 計算機

檔案：[`src/bin/q05_rpn.rs`](../src/bin/q05_rpn.rs)

## 背景

RPN（Reverse Polish Notation，後序表示法）把運算子寫在運算元後面：`3 4 +` 就是 3 + 4。
計算方式是用一個 stack：讀到數字就 push；讀到運算子就 pop 出兩個數字、算完再 push 回去。

這題的重點不是演算法，是**所有會失敗的地方都用 `Result` 回報，不准 panic**。

## 資料

```rust
enum CalcError {
    Empty,                  // 運算式裡沒有任何東西
    BadToken(String),       // 不是數字也不是運算子，帶著那個 token
    StackUnderflow,         // 運算子需要兩個數字，但 stack 裡不夠
    DivideByZero,
    Overflow,               // 運算結果超出 i64
    TooManyOperands(usize), // 算完之後 stack 裡剩不只一個數字，帶著剩幾個
}
```

## 要做的事

1. `fn eval(expr: &str) -> Result<i64, CalcError>`：以空白分隔 token，支援 `+ - * /`（整數除法）。
2. `fn pop2(stack: &mut Vec<i64>) -> Result<(i64, i64), CalcError>`：pop 出兩個數字，不夠就回傳 `StackUnderflow`。`eval` 裡用 `?` 呼叫它。
3. 幫 `CalcError` 實作 `std::fmt::Display`，每個 variant 一句給人看的訊息。
4. `main` 對下面每個運算式印一行 `<運算式> => <結果或錯誤訊息>`：

   | 運算式 | 預期 |
   |---|---|
   | `3 4 +` | `7` |
   | `5 1 2 + 4 * + 3 -` | `14` |
   | `2 3 4 * +` | `14` |
   | `7 2 /` | `3` |
   | `10 3 -` | `7` |
   | `1 +` | `StackUnderflow` |
   | `4 0 /` | `DivideByZero` |
   | `1 2` | `TooManyOperands(2)` |
   | （空字串） | `Empty` |
   | `2 x +` | `BadToken("x")` |
   | `9223372036854775807 1 +` | `Overflow` |
   | `-9223372036854775808 -1 /` | `Overflow` |

## 限制

- `eval` 和 `pop2` 裡不准出現 `unwrap`、`expect`、`panic!`
- 四則運算不准直接用 `+ - * /`，改用 `checked_add` / `checked_sub` / `checked_mul` / `checked_div`
- `eval` 裡至少有三處用到 `?`

## 實驗

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

把 `checked_add` 換回普通的 `+`，分別用 debug 和 `--release` 跑 `9223372036854775807 1 +` 那一行。看完換回來

**預測**：

**結果**：

```
debug 會顯示 overflow警告
-- thread 'main' (443131) panicked at src/bin/q05_rpn.rs:69:40:
-- attempt to add with overflow
-- note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
release 會通過但數值很怪
-- 9223372036854775807 1 + => -9223372036854775808
```

**複盤**：結果正確（預測沒填）。修兩個詞，兩個都會影響預測：

- 「overflow 警告」→ 是 **panic**，不是警告。警告（warning）是 rustc 在編譯期印的，程式照樣編得出來、跑得下去。panic 是執行期 main thread 跑到那條加法、檢查沒過，當場停下，行程以結束代碼 101 結束（實測）。
- 「數值很怪」→ 是 wrap（繞回）。`i64::MAX` 是 `0x7FFF…FF`，加 1 變成 `0x8000…00`，這個 bit pattern 當成有號數讀就是 `i64::MIN`。CPU 的加法指令本來就這樣算，release 只是沒有在後面多放那幾條「看溢位旗標」的指令。

同一行原始碼、兩種行為，就是 q01 的 `overflow-checks`：dev profile 埋檢查，release 不埋。

### 實驗 B

挑一處 `?`，改寫成功能完全相同的 `match`，確認輸出不變後改回來

**預測**：
```=rust
    "+" => {
                // num_stack.push(augend.checked_add(addend).ok_or(CalcError::Overflow)?);
                let Some(sum) = augend.checked_add(addend) else {
                    return Err(CalcError::Overflow);
                };
                num_stack.push(sum);
            }
```

**結果**：

**複盤**：行為相同，但你寫的是 `let … else`，而且直接拆 `checked_add` 回傳的 `Option`，把 `ok_or` 也一起換掉了。`?` 實際看到的是 `ok_or` 之後的 `Result`。照題目用 `match`、對著 `Result` 寫，才看得到 `?` 的每一步。

實測，換 `pop2` 那一處：

```rust
// let (addend, augend) = pop2(&mut num_stack)?;
let (addend, augend) = match pop2(&mut num_stack) {
    Ok(pair) => pair,        // 拆出裡面的值，整個 match 的值就是它，繼續往下
    Err(e) => return Err(e), // 離開 eval，把 e 交給 eval 的呼叫者
};
```

13 行輸出和原本逐行相同。（結果欄沒填。）

### 實驗 C

把 `main` 改成 `fn main() -> Result<(), CalcError>`，在裡面對一個會失敗的運算式寫 `eval("1 +")?;`。觀察畫面印出什麼，以及程式結束後 `echo $?` 的數字。（需要 `#[derive(Debug)]`）

**預測**：

```=rust
fn main() -> Result<(), CalcError> {
    // 3 4 +	7
    // 5 1 2 + 4 * + 3 -	14
    // 2 3 4 * +	14
    // 7 2 /	3
    // 10 3 -	7
    // 1 +	StackUnderflow
    // 4 0 /	DivideByZero
    // 1 2	TooManyOperands(2)
    // （空字串）	Empty
    // 2 x +	BadToken("x")
    // 9223372036854775807 1 +	Overflow
    // -9223372036854775808 -1 /	Overflow

    let list = vec![
        "3 4 +",
        "5 1 2 + 4 * + 3 -",
        "2 3 4 * +",
        "7 2 /",
        "10 3 -",
        "1 +",
        "4 0 /",
        "1 2",
        "",
        "2 x +",
        "9223372036854775807 1 +",
        "-9223372036854775808 -1 /",
        "1 +",
    ];
    for li in list {
        match eval(li) {
            Ok(v) => println!("{li} => {v}"),
            Err(e) => {
                return Err(e);
                // let question = if li.is_empty() { "(空字串)" } else { li };
                // println!("{question} => {e}")
            }
        };
    }
    Ok(())
}
```

```=bash
cargo q5 && echo $?

Error: StackUnderflow
```

**結果**：

**複盤**：`Error: StackUnderflow` 那行你看到了，結束代碼沒看到。`A && B` 的意思是「A 的結束代碼是 0 才執行 B」。`cargo q5` 失敗，`echo` 根本沒跑。`&&` 自己就是讀結束代碼的人。

改用 `;`（不管左邊成敗都執行右邊）。實測：

```
$ cargo q5; echo $?
3 4 + => 7
5 1 2 + 4 * + 3 - => 14
2 3 4 * + => 14
7 2 / => 3
10 3 - => 7
Error: StackUnderflow
1
```

| 觀察 | 怎麼驗證的 | 說明 |
|---|---|---|
| 只印了前 5 行 | 上面的輸出 | 第 6 個運算式就 `return Err(e)`，`main` 結束，後面 7 個沒跑 |
| `Error: …` 那行走 stderr，其他走 stdout | 加 `2>/dev/null` 之後那行消失，前 5 行還在 | 這行不是你的 `println!` 印的 |
| 印的是 `Debug`（`{:?}`），不是你寫的 `Display` | 把 `Display` 換成中文訊息，這行仍然是 `Error: BadToken("x")` | 你的 `Display` 字面剛好和 `Debug` 一樣，所以看不出來。這就是題目要你加 `#[derive(Debug)]` 的原因 |

是誰印的、1 是誰決定的，見問題 3。

## 問題

回答直接寫在各題下面。

### 問題 1

`?` 做了哪幾件事？用實驗 B 你寫出來的 `match` 說明。

**回答**：

```
這optional會把錯誤反饋出去，如果是真處理Ok()是假處理Err()。
```

**複盤**：太模糊，沒辦法拿來預測。先修詞：`?` 不是 optional（不是別的語言的 `?.`），也沒有真假。它處理的是 `Result`（或 `Option`）這個 enum 的兩個 variant。

`expr?` 做三件事，對照實驗 B 複盤裡的 `match`：

| 步驟 | `match` 的哪一行 | 做什麼 |
|---|---|---|
| 1 | `match expr {` | 看 `expr` 的值是哪個 variant |
| 2 | `Ok(v) => v,` | 把 `v` 拆出來。`expr?` 這個 expression 的值就是 `v`，繼續往下執行 |
| 3 | `Err(e) => return Err(e),` | **離開目前這個函式**，後面的程式碼都不跑 |

你的回答漏了 2 的「拆出值」和 3 的「提早 return」。

「反饋出去」要說清楚主詞：是從 **`?` 所在的那個函式** return 給它的呼叫者，只往上一層。`eval` 裡的 `pop2(…)?` 是把 `pop2` 回報的 `Err` 再從 `eval` return 出去。`main` 拿到之後要印還是再往上丟，是 `main` 的事。

所以 `?` 只能寫在「回傳型別裝得下這個 `Err`」的函式裡。實驗 C 要先把 `main` 改成 `-> Result<(), CalcError>` 才能寫 `eval("1 +")?;`，就是這個原因。

（實際展開是 `return Err(From::from(e))`，多一個轉型的掛鉤。這題兩邊都是 `CalcError`，等於沒轉。q07 會用到。）

### 問題 2

用了 `checked_*` 之後，debug 和 release 的行為還會不同嗎？和 q01 的溢位檢查相比，這次的檢查是誰寫的、在哪個時期執行？

**回答**：

```
debug 會檢查overflow，但是是runtime才顯示。
release 不會處理overflow警告。
```

**複盤**：答錯題了。你回答的是實驗 A（普通 `+`）的行為，題目問的是換成 `checked_*` 之後。

用了 `checked_*`，debug 和 release **行為相同**。實測 `cargo q5` 和 `cargo q5r` 的 13 行輸出一樣，溢位那兩行都是 `=> Overflow`。

| | q01、實驗 A：普通 `+` | q05：`checked_add` |
|---|---|---|
| 檢查是誰寫的 | rustc。依 profile 決定要不要在加法指令後面埋 | 你。你呼叫了 `checked_add`，它的函式本體就是「加，然後看有沒有溢位」 |
| 檢查在不在 binary 裡 | dev 有、release 沒有 | 兩個 profile 都有。它是你程式邏輯的一部分 |
| 何時執行 | 執行期 | 執行期 |
| 溢位時 | dev：panic。release：wrap，拿錯的數字繼續算 | 回傳 `None`，由你決定把它變成 `Err(Overflow)` |
| 呼叫者能處理嗎 | 不能 | 能。它是一個普通的值 |

你的模型在這裡會預測錯：「release 不處理 overflow」會推出 release 版的計算機印 `-9223372036854775808`。不會。

> release 關掉的只有 rustc **自動**埋的那種檢查。你自己寫的 `if`、你自己呼叫的函式，rustc 不會拿掉。

### 問題 3

實驗 C 的那個結束代碼是誰設定的？從「`main` 回傳 `Err`」到「shell 的 `$?` 拿到數字」，中間經過哪幾層？

**回答**：

```
猜想一:
fn main() -> Result<(), CalcError>
Result 中 
Ok() 反饋stdout
Err() 反饋stderr

猜想二:
cargo q5 && echo $?
process結束的signal就是回傳值?(猜測，但這跟我上面猜測反例。)
```

**複盤**：實驗 C 沒看到數字，兩個猜想都沒有東西可以驗。實測是 `1`。這一節依後來的討論重寫過。

#### 一句話版本

`main` 回傳的 `Err` 被 std 的 runtime 翻譯成數字 `1`，這個數字經過 kernel，一手一手傳給父行程，最後被 shell 抄進 `$?`。全程不經過任何 fd。

#### 先看角色：誰在哪裡

```
你的 process（一個 process，一條 main thread）
┌──────────────────────────────────────────────┐
│ C runtime 的啟動碼      ← process 開始時最先跑  │
│   └─ 呼叫 std 的 runtime（std::rt，Rust 寫的） │
│         └─ 呼叫你的 main                      │
│         ┌─ main return Err(e)                │
│      ←──┘ runtime 印 Error、決定數字 1         │
│   ←──── 把 1 交回 C runtime                   │
│ C runtime 呼叫 exit(1) ── syscall ──┐         │
└─────────────────────────────────────┼────────┘
                                      ▼
                           kernel（不是 process）
                           記下 1，拆掉這個 process，
                           叫醒睡在 wait 裡的父行程
```

| 角色 | 是什麼 | 在哪 | 比喻（員工離職） |
|---|---|---|---|
| 你的 `main` | 你寫的函式 | 你的 process，user mode | 員工，交出一份報告（`Result`） |
| std runtime | 編進你 binary 的函式 | 同一個 process，user mode | 主管，看報告、喊一句結論、定一個代碼 |
| C runtime | 連結進你 binary 的函式 | 同一個 process，user mode | 人資，拿代碼去辦離職手續 |
| kernel | 作業系統本體 | 不屬於任何 process，kernel mode | 政府機關，登記「已離職，代碼 1」 |
| cargo | 另一個 process | 你的 process 的父行程 | 派員工來的仲介，在機關門口等 |
| shell | 另一個 process | cargo 的父行程 | 委託仲介的客戶 |

前三個是**同一條 thread 依序跑三段不同人寫的函式**，和 `main` 呼叫 `eval`、`eval` 呼叫 `pop2` 一樣，只是函式呼叫。C runtime 呼叫 Rust 這一段時，雙方遵守的約定是 C 的 ABI。真正「別的 process」只有 cargo 和 shell。

#### 數字怎麼傳：時間軸

```
shell ──開──► cargo ──開──► q05_rpn
  ▲ wait        ▲ wait
  └── 1 ────────┴── 1 ─────── exit(1)
```

```
t    誰的程式碼在跑    mode     發生什麼
t0   你的 main        user     return Err(StackUnderflow)。只是把值交給呼叫者，不知道什麼代碼
t1   std runtime      user     它是呼叫 main 的人。看回傳值：
                                Ok(())  → 代碼 0，什麼都不印
                                Err(e)  → 往 fd 2 寫 "Error: {e:?}"，代碼 1
t2   C runtime        user     收到 1，呼叫 exit(1)，thread 進 kernel
t3   kernel           kernel   把 1 填進這個 process 的紀錄，拆掉它（關 fd、收回記憶體），
                                查紀錄上的父行程是 cargo，把睡在 wait 裡的 cargo 叫醒
t4   cargo            user     wait 回傳，帶著 1。cargo 自己也 exit(1)
t5   kernel           kernel   同 t3：填 cargo 的紀錄，叫醒它的父行程 shell
t6   shell            user     wait 回傳，帶著 1。shell 把 1 存進自己的變數 ?
t7   shell            user     你打 echo $?，shell 把 $? 換成 1，印出來
```

三個重點：

- **接力**。你的程式不認識 shell，shell 也看不到你程式裡的 `Err`。每一手只認識上下兩家，沒有一個中央管理者在「把結果送給 shell」。
- **mode 只在 t3、t5 變成 kernel**。「結束一個 process」只有 kernel 能做，程式只能申請。
- **等的人是睡著的**。cargo 呼叫 `wait` 之後 thread 就睡了，不佔 CPU，不是一直在問「好了沒」。是 kernel 在 t3 把它叫醒。

誰設定的：`1` 是 std runtime 在 t1 決定的。不是你的 `main`（它只回傳一個 `Result`），也不是 kernel（它只保管和轉交）。

#### kernel 靠什麼知道要叫醒誰

kernel 裡每個 process 有一筆紀錄（kernel 的資料，不在任何 process 的記憶體裡）：

```
┌───────────────────────────┐
│ pid: 5002  （q05_rpn）      │
│ 父行程: 5001（cargo）       │ ← 開子行程的那一刻就填好
│ 結束代碼: （還沒有）         │ ← exit(1) 時才填
└───────────────────────────┘
```

靠的是「父子關係」這條線，不是 fd。

| | 等 IO（note 02） | 等子行程結束（這題） |
|---|---|---|
| 等什麼 | 某個 fd 可以讀寫了 | 某個子行程結束了 |
| 用哪個 syscall 等 | `epoll_wait` / `kevent` | `wait` 系列 |
| kernel 靠什麼知道要叫誰 | fd 上掛的等待名單 | process 紀錄裡的「父行程」 |
| 拿到什麼 | 「這個 fd 好了」 | 子行程的結束代碼 |

這也是 process 之間傳東西（IPC）最原始的一種，但路很窄：只能子 → 父、一輩子一次、內容是一個小整數（0–255）。要傳更多得用 pipe、檔案、socket。

#### 兩條管道：字和數字是分開的

| | 內容 | 怎麼出去 | 誰收 |
|---|---|---|---|
| fd 1（stdout） | `3 4 + => 7` 那些行 | `write(1, …)` | 終端機、`>`、`\|` |
| fd 2（stderr） | `Error: StackUnderflow` | `write(2, …)` | 終端機、`2>` |
| 結束代碼 | 整數 `1` | `exit(1)` 的參數 | 父行程，用 `wait` 領 |

t1 做了兩件**獨立**的事：往 fd 2 寫一行字，然後帶著 1 結束。`2>/dev/null` 把字丟掉，`$?` 還是 1。

#### `$?` 是什麼

shell 自己的一個變數，存在 shell 這個 process 的記憶體裡。每次有子行程結束，shell 就用 `wait` 領到的數字覆蓋它一次。所以 `echo $?` 連打兩次，第二次一定是 0：那是第一個 `echo` 的結束代碼。

`A && B`、`A ; B` 也是 shell 在判斷，讀的就是這個數字。

#### `Debug` 和 `Termination`

`main` 的回傳型別必須實作 `Termination`（終止）這個 trait，t1 的 runtime 就是呼叫它來拿代碼。這份 impl **已經存在**，是 std 寫的：

```rust
impl<T: Termination, E: Debug> Termination for Result<T, E> { … }
```

念作：「任何 `Result<T, E>`，只要 `E` 有 `Debug`，就自動有 `Termination`」。`Debug` 是條件，因為 t1 要用 `{:?}` 印它。

| 你做的事 | 結果 |
|---|---|
| `CalcError` 加 `#[derive(Debug)]` | 條件滿足，`main` 可以回傳 `Result<(), CalcError>` |
| 拿掉 `Debug` | E0277：`required for Result<(), CalcError> to implement Termination` |

你不能自己寫 `impl Termination for Result<(), CalcError>`：trait 和 `Result` 都是 std 的，兩個都不是你的。和 `impl i64`、`Drop for u32`、`Display for Result` 是同一條規則。

這整套只對 `main` 生效。一般函式回傳 `Err` 就只是把值交給呼叫者。

#### 誰能啟動一個執行檔

shell 不是必經的一關。必經的只有：**某個已經在跑的 process 向 kernel 申請開子行程**。

| 誰啟動 | 例子 |
|---|---|
| shell | 在終端機打 `./q05_rpn` |
| 另一個程式 | cargo 啟動你的 binary；編輯器的 rust-analyzer 存檔時直接啟動 `cargo check` |
| 你自己的程式 | `std::process::Command::new("…").status()`，這一行就是「開子行程 + wait」 |
| 系統服務管理員 | macOS 的 launchd、Linux 的 systemd |

不能的是「沒有人啟動」。每個 process 都有父行程，往上追到 pid 1 為止，它是 kernel 開機時自己建的：

```
q05_rpn ← cargo ← shell ← 終端機 app ← launchd（pid 1）← kernel
```

自己看父行程：

```rust
println!("pid {}，父行程 {}", std::process::id(), std::os::unix::process::parent_id());
```

```
ps -o pid,ppid,comm -p <父行程 pid>
```

#### 陷阱清單

1. ❌ `Ok` 反饋 stdout、`Err` 反饋 stderr → ✅ `Ok(())` 什麼都不印；`Err` 時往 stderr 寫的那行只是 runtime 順口喊的，結果是靠結束代碼傳的 → 為什麼：字和數字是兩條分開的管道。
2. ❌ process 結束的 signal 就是回傳值 → ✅ 結束時交出去的是結束代碼（exit status）；signal 是 kernel 送給 process 的通知（例如 Ctrl-C） → 為什麼：兩套不同的機制。
3. ❌ kernel、Rust、C 是三種方式（或三個 process） → ✅ Rust runtime 和 C runtime 是同一個 process 裡的兩段函式，kernel 不是 process → 為什麼：前者是同一條 thread 的函式呼叫，進 kernel 才是 syscall。
4. ❌ kernel 調整 fd，cargo 因此收到結束 → ✅ kernel 把代碼填進 process 紀錄，照紀錄上的「父行程」叫醒 cargo → 為什麼：等子行程結束用的是 `wait`，不是盯 fd。kernel 關 fd 只是打掃。
5. ❌ cargo 一直在 listen → ✅ cargo 的 thread 睡在 `wait` 裡，不佔 CPU → 為什麼：等待不是在做事，是被叫醒之前都不跑。
6. ❌ 因為用了 `Debug`，所以沒有 impl `Termination` → ✅ impl 是 std 早就寫好的，`Debug` 是它的條件 → 為什麼：`impl<…, E: Debug> Termination for Result<T, E>`。
7. ❌ 執行檔一定要透過 shell 啟動 → ✅ 任何 process 都能開子行程 → 為什麼：shell 只是「讓人用打字申請開子行程」的工具。
8. ❌ `cargo q5` 時你的程式的父行程是 shell → ✅ 是 cargo；shell 的子行程是 cargo → 為什麼：誰開的誰就是父行程。

### 問題 4

這題裡哪些情況適合用 `Result`？有沒有哪種情況你認為直接 panic 才合理？判斷標準是什麼？

**回答**：

```
允許的錯誤使用Result，異常的錯誤需要panic(panic是process中斷1)。
這題為例，我覺得都使用Result較佳，因為可以處理N個題目，單一題目錯誤不至於讓process中斷，且業務上也沒有關聯性。
```

**複盤**：結論對，這題六種失敗都該是 `Result`。判斷標準補精確：看**是誰造成的**。

| | `Result` | panic |
|---|---|---|
| 誰造成的 | 輸入或環境：使用者打錯、檔案不存在 | 程式自己寫錯：不該出現的狀態出現了 |
| 預期會發生嗎 | 會，是正常流程的一部分 | 不會，發生就代表有 bug |
| 誰處理 | 呼叫者 | 沒有人。要改的是程式碼 |

六個 variant 全都由 `expr` 的內容決定，所以全是 `Result`。

你的程式裡其實有一處屬於右邊那欄：內層 `match token` 的 `_ => return Err(CalcError::BadToken(…))`。外層已經確認 token 是四個運算子之一，會走到這條只可能是程式被改壞（例如外層加了 `"%"`、內層忘了加），不是使用者打錯。這時回報 `BadToken("%")`，是把自己的 bug 說成使用者的錯。這種位置才是 `unreachable!()` 合理的地方。更好的做法是讓這條 arm 不存在，見程式碼複盤第 1 點。

修一個數字：panic 的結束代碼是 101，不是 1（實測）。1 是 `main` 回傳 `Err`。另外 panic 停下的是**那條 thread**（沿路照樣 drop）。main thread panic 之後，是 runtime 讓整個行程以 101 結束。

### 問題 5

`BadToken(String)` 為什麼帶的是 `String` 而不是 `&str`？如果要改成 `&str`，`CalcError` 和 `eval` 的簽名會變成什麼樣子？

**回答**：

```
恩，我也想知道其中奧妙。

猜測：
這邊使用String希望作用是move，
往這方面去猜就是期望每個frame都可以正常釋放，
如果使用&str的話，
反而是某一次的frame內的&str一直binding，
但如果是使用String當錯誤拋到最外層時候，
內層frame也都釋放了。
```

**複盤**：方向對（自己擁有的資料才不受別人的生死影響），但卡在老地方：把 lifetime 當成 frame。

模型錯的地方：「`&str` 會綁在某一次 frame 內」。`token` 不指向 `eval` 的 frame。`split_whitespace()` 切出來的每個 `&str` 是 (ptr, len)，ptr 指進 **`expr` 那串字元**裡面。那串字元的擁有者是 `eval` 的呼叫者（這題是字串字面值，在 `.rodata`）。`eval` return 時消失的只有 (ptr, len) 這 16 bytes，被指的字元還在。

所以 `&str` 版**編得過**（實測，12 行輸出相同）：

```rust
enum CalcError<'a> {
    Empty,
    BadToken(&'a str),
    // 其餘不變
}

fn eval<'a>(expr: &'a str) -> Result<i64, CalcError<'a>>
// 省略寫法：fn eval(expr: &str) -> Result<i64, CalcError<'_>>
```

`'a` 的意思：「回傳的 `CalcError` 裡借的東西，和 `expr` 借的是同一份。`expr` 指的那串字元活多久，這個 error 最多就能用多久。」它不決定任何東西何時釋放，只是寫給 rustc 檢查用的。

會失敗的是輸入字串的擁有者比 error 早消失：

```rust
fn read_and_eval() -> Result<i64, CalcError<'static>> {
    let line = String::from("2 x +"); // 字元的擁有者在這個 frame
    eval(&line)
    // error[E0515]: cannot return value referencing local variable `line`
}
```

你說的「frame 釋放」在這裡才成立，而且是**輸入字串擁有者**的 frame，不是 `eval` 的。

| | `BadToken(String)` | `BadToken(&'a str)` |
|---|---|---|
| error 裡放的 | 自己擁有的一份字元，在 heap | 指進輸入字串的 (ptr, len) |
| 成本 | 出錯時配置一次 heap | 不配置 |
| 型別 | `CalcError` | `CalcError<'a>`，所有提到它的簽名都要帶 lifetime |
| error 能活多久 | 不受限：存起來、一路往上丟、送到別條 thread | 不能超過輸入字串 |

題目選 `String` 的理由：error 通常會被一路往上丟，離輸入越來越遠。只在失敗那條路多配置一次，換來 error 不依賴任何人。

## 加分

回到 q03，把 `pick_up` 的回傳型別從 `bool` 改成 `Result<(), Item>`：放不下時把物品還給呼叫端，而不是讓它消失。

```
改一下 pick_up -> pick_up_2
```

**複盤**：完成。`pick_up_2` 放不下時 `return Err(item)`，物品回到呼叫者手上。輸出裡有證據：

```
drop: anvil
pickup anvil result is Can't pickup Item: anvil
```

`drop: anvil` 還在，但這次 drop 它的是 `main`：`Err(v)` 把 anvil 綁到 `v`，`v.to_string()` 做出字串，arm 結束，`v` 離開 scope。原版是在 `pick_up` 的 `return false` 被 drop，呼叫者沒得選。現在是 `main` 拿到之後自己選擇不留。

🟡 建議

- `#[allow(dead_code)]` 貼在 `pick_up_2` 上是多的，它有被呼叫。舊的 `pick_up` 和 `main` 裡註解掉的那段直接刪。
- `Display for Item` 寫成 `Can't pickup Item: …`，把「撿不起來」這個情境寫死進 `Item` 的顯示方式，之後任何地方印 `Item` 都會帶這句。和 q04 `parse` 自己印字是同一類問題：這句話由 `main` 的 `Err(v)` arm 來組。
- `self.capacity - self.total_weight()` 的 `u32` 減法還在（q03 複盤提過）。

## 程式碼複盤

### 第五輪：通過，結案

輸出 13 行和預期逐行相同（debug、release 都是），`cargo clippy` 沒有 warning。

✅ 這一輪解決的

- 第四輪的 🔴：手寫的 `match … Err(e) => return Err(e)` 換成 `?`。

  ```rust
  num_stack.push(Op::calc(&op, augend, addend)?)
  ```

  `?` 寫在 `push(…)` 的括號裡，`return` 離開的仍然是整個 `eval`，`push` 不會被呼叫。`4 0 /`、兩行 `Overflow` 都還印得出來，就是 `Err` 有經過這個 `?` 傳到 `main` 的證據。

  > `?` = 成功就給我裡面的值，失敗就替我 `return Err`。

三條限制：`eval` / `pop2` 沒有 `unwrap` / `expect` / `panic!`；四則都用 `checked_*`；`eval` 裡 `?` 有 3 處。

🟡 沒改的建議（不影響結案）

- `Op::calc(&op, …)` → `op.calc(…)`；`match *self` → `match self`。
- `None` 那條的 `if let Ok(iteger) … else { return Err(…) }` 是手寫的 `?`，可以寫成 `map_err(…)?`。
- `addend` / `augend` 命名與 `pop2` 的回傳順序（右, 左）。
- 結尾 `let rs = …?; Ok(rs)`。
- `Display` 和 `Debug` 印出來相同。
- `main` 的 `vec!`、第 13 個 `"1 +"`、開頭的註解。
- 實驗 B、C 的「結果」沒有填，實測結果都寫在各自的複盤裡。

沒答的兩題（不擋結案，答案可以用 `cargo` 驗證）

- (a) `pop2` 裡寫 `let rhs = stack.pop()?;`（不加 `ok_or`），編得過嗎？
- (b) 問題 5 的 `&str` 版，每一輪 `let line = String::from(li);` 再 `eval(&line)`。迴圈裡直接印 `e` 編得過嗎？把 `e` push 進迴圈外的 `Vec` 呢？

### 第四輪：根因解掉了，`?` 少一處

輸出 13 行全對，debug、release 都是。

✅ 這一輪解決的

- 前三輪一直在換寫法的那件事，這次真的解了。`Op` 不再帶運算元，`Op::parse(token)` 只看字串，「是不是運算子」只問一次：`Some(op)` 才 `pop2`，`None` 去試數字。外層的 `"+" | "-" | "*" | "/"` 拿掉了，那條「不可能」的路（`_ => BadToken` → `unwrap()` → `ok_or(BadToken)?`）不存在了。
- 第三輪 🟡 1 的多餘配置也跟著消失：遇到運算子不再先做一個用不到的 `BadToken`。
- `calc` 的運算元在計算時才給，四個 variant 全列，沒有 `_`。
- 註解掉的舊 `match token` 刪了。

🔴 必改

1. `eval` 裡的 `?` 剩 2 處（`pop2(…)?` 和結尾的 `ok_or(…)?`），限制第 3 條要至少 3 處。第三輪的第 3 處是那條 `ok_or(BadToken)?`，它被（正確地）刪掉之後就少了一處。該補上的位置第二輪、第三輪都指過：

   ```rust
   match Op::calc(&op, augend, addend) {
       Ok(v) => num_stack.push(v),
       Err(e) => return Err(e),
   }
   ```

   對照問題 1 的表：看 variant、`Ok(v)` 拆出 `v`、`Err(e)` 就 `return Err(e)`。三步一模一樣，這就是 `?`。clippy 仍然在報 `this match expression can be replaced with ?`。換成一行，實測輸出相同、clippy 沒有 warning：

   ```rust
   num_stack.push(op.calc(augend, addend)?);
   ```

   執行順序：main thread 先算 `op.calc(…)`，`?` 看結果。`Err` 就直接離開 `eval`，`push` 不會被呼叫；`Ok(v)` 才把 `v` 交給 `push`。

🟡 建議（不擋結案）

- `Op::calc(&op, …)` → `op.calc(…)`，上面那行已經一起改了。
- `match *self`：`Op` 沒有欄位，和 `match self` 沒有差別（note 01 §4）。習慣寫 `match self`。
- `None` 那條的 `if let Ok(iteger) … else { return Err(…) }` 也是手寫的 `?`。改成 `map_err(…)?` 的話 `?` 就有 4 處。
- 第一輪其餘的 🟡 仍在：`addend` / `augend` 命名與 `pop2` 回傳順序、結尾 `let rs`、`Display` 與 `Debug` 相同、`main` 的 `vec!`、第 13 行、開頭的註解。

**還沒結案，剩三件**

1. 上面那條 🔴（改一行）。
2. 實驗 C 用 `cargo q5; echo $?` 自己跑一次，把結果填上。（還沒填）
3. 這兩題先預測再跑，答案寫在下面：（還沒答）
   - (a) `pop2` 裡寫 `let rhs = stack.pop()?;`（不加 `ok_or`），編得過嗎？用問題 1 的展開式說明。
   - (b) 問題 5 的 `&str` 版，`main` 改成每一輪 `let line = String::from(li);` 再 `eval(&line)`。在迴圈裡直接印 `e`，編得過嗎？把 `e` push 進迴圈外的 `Vec` 呢？

### 第三輪：限制過了，根因沒動

輸出 13 行全對（debug、release 都是）。兩條限制照字面都守住了：`eval` 沒有 `unwrap`，`?` 有 3 處。

但第二輪 🔴 1 講的那件事沒有改，只是又換了一個寫法：

```rust
let op = Op::parse(token, augend, addend)
    .ok_or(CalcError::BadToken(token.to_string()))?;
```

| | 第一輪 | 第二輪 | 第三輪 |
|---|---|---|---|
| 第二次問「是不是運算子」 | 內層 `match token` | `Op::parse` | `Op::parse` |
| 「不可能」的那條路 | `_ => return Err(BadToken)` | `unwrap()` | `ok_or(BadToken)?` |
| 真的走到時 | 把自己的 bug 報成使用者打錯 | panic | 把自己的 bug 報成使用者打錯 |

第三輪等於第一輪。外層 `"+" | "-" | "*" | "/"` 已經保證 `parse` 一定回 `Some`，這個 `ok_or` 的錯誤永遠用不到。

🟡 建議（不擋結案）

1. 這次的寫法多了一個成本。函式的參數會在呼叫**之前**先算好，所以 `ok_or(CalcError::BadToken(token.to_string()))` 每遇到一個運算子，都先 `to_string()` 配置一塊 heap、做出一個 `BadToken`，然後 `ok_or` 發現是 `Some`，把它丟掉。實測：

   ```rust
   let _ = Some(1).ok_or(make());           // make() 被呼叫
   let _ = Some(1).ok_or_else(|| make());   // make() 沒被呼叫
   ```

   `ok_or_else` 收的是 closure，只在 `None` 時才執行。規則：錯誤值要配置或要計算才做得出來，就用 `ok_or_else`；像 `CalcError::Overflow` 這種不花成本的，`ok_or` 就好。不過這裡真正的解法是讓這個 `ok_or` 不存在。
2. 根因的解法和第二輪寫的一樣：運算元從 `Op` 拿出來（`enum Op { Add, Sub, Mul, Div }`），`parse(token)` 只看字串，外層那條 `"+" | "-" | "*" | "/"` 拿掉，由 `Op::parse` 當唯一的判斷。
3. `match Op::calc(&op) { Ok(v) => …, Err(e) => return Err(e) }` 還是手寫的 `?`，clippy 仍然有這條 warning。寫成 `num_stack.push(op.calc()?);`。
4. 第一輪的 🟡 都還在。

**還沒結案，剩兩件**（都不是程式碼）

1. 實驗 C 用 `cargo q5; echo $?` 自己跑一次，把結果填上。
2. 下面兩題先預測再跑，答案寫在這裡：
   - (a) `pop2` 裡寫 `let rhs = stack.pop()?;`（不加 `ok_or`），編得過嗎？用問題 1 的展開式說明。
   - (b) 問題 5 的 `&str` 版，`main` 改成每一輪 `let line = String::from(li);` 再 `eval(&line)`。在迴圈裡直接印 `e`，編得過嗎？把 `e` push 進迴圈外的 `Vec` 呢？

### 第二輪：未通過

輸出 13 行仍然全對（debug、release 都是）。但加了 `Op` 之後，兩條限制破了：

🔴 必改

1. `eval` 裡出現 `unwrap`（限制第 1 條）：

   ```rust
   let op = Op::parse(token, augend, addend).unwrap();
   ```

   第一輪那條走不到的 `_ => BadToken` 沒有消失，只是搬到這裡變成 `unwrap()`。原因相同：「是不是運算子」問了兩次。外層 `"+" | "-" | "*" | "/"` 問一次，`Op::parse` 裡的 `match token` 又問一次。第二次的答案你知道一定是 `Some`，但型別是 `Option`，只好 `unwrap`。

   | | 第一輪 | 第二輪 |
   |---|---|---|
   | 第一次問 | 外層 `match token` | 外層 `match token` |
   | 第二次問 | 內層 `match token` | `Op::parse` 裡的 `match token` |
   | 第二次「不可能」的那條路 | `_ => return Err(BadToken)` | `None` → `unwrap()` panic |

   > 只問一次。讓 `Op::parse` 當那個唯一的判斷：`Some(op)` 就是運算子，`None` 就去試數字。外層的 `"+" | "-" | "*" | "/"` 整條拿掉。

   你現在做不到，是因為 `Op` 帶著兩個運算元：`parse` 需要 `augend` / `addend`，所以得先 `pop2`，而要不要 `pop2` 又得先知道是不是運算子，於是外層那次判斷拿不掉。把運算元從 `Op` 拿出來就解開了：

   ```rust
   enum Op { Add, Sub, Mul, Div }           // 只記「哪一種運算」

   fn parse(token: &str) -> Option<Op>      // 只看字串
   fn calc(self, lhs: i64, rhs: i64) -> Result<i64, CalcError>   // 運算元在算的時候才給
   ```

   ```rust
   let value = match Op::parse(token) {
       Some(op) => {
           let (lhs, rhs) = pop2(&mut num_stack)?;
           op.calc(lhs, rhs)?
       }
       None => /* 試數字，失敗才是 BadToken */,
   };
   num_stack.push(value);
   ```

2. `eval` 裡的 `?` 剩 2 處（限制第 3 條要至少 3 處）。少掉的那些變成了手寫版：

   ```rust
   match Op::calc(&op) {
       Ok(v) => num_stack.push(v),
       Err(e) => return Err(e),
   }
   ```

   這就是實驗 B 的 `match`，clippy 也指出來了（`this match expression can be replaced with ?`）。寫成 `num_stack.push(op.calc()?);`。

🟡 建議

- `Op::calc(&op)` → `op.calc()`。第一個參數是 `&self` 的 fn 用點呼叫，rustc 會自動幫你借。q04 的 `Command::execute(cmd, …)` 是同一件事。
- 註解掉的舊 `match token`（17 行）刪掉。
- 第一輪的 🟡 都還在：`addend` / `augend` 命名、`pop2` 回傳順序、`if let … else` 手寫的 `?`、結尾 `let rs`、`Display` 與 `Debug` 相同、`main` 的 `vec!` 和第 13 行。

🟢 做得好

- `calc` 的 `match *self` 用對了：`Op::Add(augend, addend)` 綁出來的是 `i64`（`Copy`），所以可以直接 `.checked_add`，不用再 `*`。這是 note 01 §4 剛補的那條規則。
- `calc` 四個 variant 全列，沒有 `_`。
- 除以零和溢位在 `Op::Div` 裡仍然分開。

**還沒結案，剩三件**

1. 上面兩條 🔴。
2. 實驗 C 用 `cargo q5; echo $?` 自己跑一次，把結果填上。（還沒填）
3. 這兩題先預測再跑，答案寫在下面：（還沒答）
   - (a) `pop2` 裡寫 `let rhs = stack.pop()?;`（不加 `ok_or`），編得過嗎？用問題 1 的展開式說明。
   - (b) 問題 5 的 `&str` 版，`main` 改成每一輪 `let line = String::from(li);` 再 `eval(&line)`。在迴圈裡直接印 `e`，編得過嗎？把 `e` push 進迴圈外的 `Vec` 呢？

### 第一輪

**程式碼通過**（當時的版本）：

- 12 行輸出和預期逐行相同，debug、release 都是。多出來的第 13 行 `1 +` 是實驗 C 留下的。
- `cargo clippy` 沒有 warning。
- 三條限制都守住：`eval` / `pop2` 沒有 `unwrap` / `expect` / `panic!`；四則都用 `checked_*`；`eval` 裡 `?` 有 6 處。

🔴 必改：沒有。

🟡 建議

1. 內外兩層 `match token`，內層被迫多一條走不到的 `_`（問題 4）。攤平成一層：每條 arm 算出一個值，最後統一 `push`。走不到的 arm 消失，你也不必編一個假的錯誤給 rustc。實測輸出相同：

   ```rust
   for token in expr.split_whitespace() {
       let value = match token {
           "+" => {
               let (lhs, rhs) = pop2(&mut stack)?;
               lhs.checked_add(rhs).ok_or(CalcError::Overflow)?
           }
           // "-"、"*" 同形
           "/" => {
               let (lhs, rhs) = pop2(&mut stack)?;
               if rhs == 0 {
                   return Err(CalcError::DivideByZero);
               }
               lhs.checked_div(rhs).ok_or(CalcError::Overflow)?
           }
           _ => token
               .parse::<i64>()
               .map_err(|_| CalcError::BadToken(token.to_string()))?,
       };
       stack.push(value);
   }
   ```

2. 命名。`addend` / `augend` 只對加法成立。而且 `pop2` 回傳的順序是（右, 左），呼叫端要記得反過來寫 `augend.checked_sub(addend)`。現在是對的，但 `-` 和 `/` 的順序一寫反結果就錯，rustc 不會提醒。回傳 `(lhs, rhs)`，照算式的順序。
3. `pop2` 可以用這題的主角寫：`let rhs = stack.pop().ok_or(CalcError::StackUnderflow)?;`
4. `if let Ok(n) = token.parse::<i64>() { push } else { return Err(…) }` 是手寫的 `?`。`map_err` 把 `parse` 自己的錯誤型別換成你的，後面接 `?`（見第 1 點的最後一條 arm）。
5. 結尾 `let rs = num_stack.pop().ok_or(CalcError::Empty)?; Ok(rs)`：拆開又包回去。`num_stack.pop().ok_or(CalcError::Empty)` 本身就是 `Result<i64, CalcError>`，直接當最後一個 expression。和 q04 的 `let rs = match …; rs` 是同一個習慣。
6. `Display` 和 `Debug` 印出來一模一樣，等於沒分工。`Debug`（`{:?}`）給開發者看，derive 的就夠。`Display`（`{}`）給使用者看，題目要的是「一句給人看的訊息」，例如 `除數是 0`、`看不懂的 token：x`。
7. `main`：`vec![…]` 用 array `[…]` 就好，不需要 heap。第 13 個 `"1 +"` 和開頭那段註解直接刪。`iteger` → `integer`。

🟢 做得好

- 除以零和溢位分開：先 `if addend == 0` 回 `DivideByZero`，剩下的 `None` 才是 `Overflow`。這是你寫的時候卡住的地方：`checked_div` 的 `None` 不帶原因，原因是你判斷的。`-9223372036854775808 -1 /` 那行對了就是證明。
- 結尾先查 `len() > 1`，再 `pop().ok_or(Empty)`：剩太多、沒東西、剛好一個，三種結局各走各的，沒有 `unwrap`。
- `for token in expr.split_whitespace()` 直接走訪 iterator，沒有 `collect` 成 `Vec`，比 q04 少一次 heap 配置。
- `pop2` 的參數是 `&mut Vec<i64>`：借進來改，不拿走。
- 空字串那行另外印 `(空字串)`，輸出不會只剩一個 `=>`。
- 加分題有做。

## 交卷

commit 之後說「複盤 q05」。
