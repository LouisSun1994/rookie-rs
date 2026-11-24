# q04：指令解析器

檔案：[`src/bin/q04_commands.rs`](../src/bin/q04_commands.rs)

## 背景

`enum` 的每個 variant 可以各自帶不同的資料，`match` 會強迫你處理每一種情況。
`Option` 本身就是一個 enum，用來表示「可能沒有」。

## 資料

```rust
enum Command {
    Add(String, u32), // 物品名稱、數量
    Remove(String),
    List,
    Quit,
}
```

## 要做的事

1. `fn parse(line: &str) -> Option<Command>`：把一行文字轉成 `Command`，無法解析就回傳 `None`。

   | 輸入 | 結果 |
   |---|---|
   | `add sword 2` | `Some(Add("sword", 2))` |
   | `remove sword` | `Some(Remove("sword"))` |
   | `list` | `Some(List)` |
   | `quit` | `Some(Quit)` |
   | 其他（指令不存在、缺參數、數量不是數字） | `None` |

2. `fn execute(cmd: Command, bag: &mut Vec<(String, u32)>) -> bool`：執行指令，回傳「是否繼續」（`Quit` 回傳 `false`）。
   - `Add`：背包裡已有同名物品就把數量加上去，否則新增一筆
   - `Remove`：移除該物品；不存在就印 `找不到: <名稱>`
   - `List`：依加入順序，每筆印一行 `<名稱> x<數量>`
3. `main` 對下面這組輸入逐行處理：先印 `> <該行>`，再解析並執行；解析失敗印 `無法解析`；`execute` 回傳 `false` 就停止。

   ```rust
   let lines = [
       "add sword 2",
       "add potion 5",
       "add sword 1",
       "list",
       "remove potion",
       "remove dragon",
       "fly away",
       "add shield",
       "add shield many",
       "list",
       "quit",
       "add ghost 1",
   ];
   ```

   預期輸出：

   ```
   > add sword 2
   > add potion 5
   > add sword 1
   > list
   sword x3
   potion x5
   > remove potion
   > remove dragon
   找不到: dragon
   > fly away
   無法解析
   > add shield
   無法解析
   > add shield many
   無法解析
   > list
   sword x3
   > quit
   ```

## 限制

- `execute` 裡的 `match` 要把四個 variant 全部列出來，不准用 `_ =>`
- `parse` 裡不准出現 `unwrap` / `expect`
- 可以用 `split_whitespace()`、`.next()`、`.parse::<u32>().ok()`

## 實驗

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

在 `Command` 加一個 variant `Clear`，先不要改 `execute`，直接編譯。看完結果再把 `Clear`（清空背包）做完

**預測**：

```=rust
enum Command {
    Add(String, u32), // 物品名稱、數量
    Remove(String),
    List,
    Quit,
    Clear,
}

impl Command {
    fn parse(line: &str) -> Option<Command> {
        match line.split_whitespace().collect::<Vec<&str>>().as_slice() {
            ["add", item, count] if let Ok(count_32) = count.parse::<u32>() => {
                Some(Command::Add(String::from(*item), count_32))
            }
            ["remove", item] => Some(Command::Remove(String::from(*item))),
            ["list"] => Some(Command::List),
            ["quit"] => Some(Command::Quit),
            _ => {
                println!("無法解析");
                None
            }
        }
    }
    fn execute(cmd: Command, bag: &mut Vec<(String, u32)>) -> bool {
        let rs = match cmd {
            Command::Add(item, count) => {
                println!("{item} x{count}");
                match bag.iter_mut().find(|f| f.0 == item) {
                    Some((_, quantity)) => *quantity += count,
                    None => bag.push((item, count)),
                }
                true
            }
            Command::Remove(item) => {
                match bag.iter().position(|f| f.0 == item) {
                    Some(idx) => {
                        bag.remove(idx);
                    }
                    None => println!("No item {item}"),
                }
                println!("remove {item}");
                true
            }
            Command::List => {
                bag.iter().for_each(|(k, v)| println!("{k} x{v}"));
                true
            }
            Command::Quit => false,
            Command::Clear => {
                bag.clear();
                true
            }
        };
        rs
    }
}

```

**結果**：

**複盤**：「預測」欄貼的是做完之後的程式碼，沒有寫預測。你的預測其實在問題 2 的回答裡（「compile 會反饋錯誤」），是對的。只加 variant、不動 `execute` 的實測：

```
error[E0004]: non-exhaustive patterns: `Command::Clear` not covered
   |
   |         let rs = match cmd {
   |                        ^^^ pattern `Command::Clear` not covered
help: ensure that all possible cases are being handled by adding a match arm
      with a wildcard pattern or an explicit pattern as shown
   |
   ~             Command::Quit => false,
   ~             Command::Clear => todo!(),
```

第三輪已補上 `["clear"]`，`clear` 之後 `list` 是空的，實驗完成。下面保留第二輪的狀況。

第二輪：variant 和 `execute` 已經進 `.rs`，但`parse` 沒有 `["clear"]` 這一條，沒有任何輸入能產生 `Command::Clear`。程式編得過、跑得動，`lines` 裡也沒有 `clear`，所以看不出問題。rustc 只給 warning，不是 error：

```
warning: variant `Clear` is never constructed
```

把 `"clear"` 和 `"list"` 加進 `lines` 就看得到：

```
> list
sword x3
> clear
無法解析          ← 指令明明存在，卻走到「無法解析」
> list
sword x3          ← 背包沒被清空
```

| 時間 | 誰在跑 | 發生什麼 |
|---|---|---|
| t0 | `main` | 印 `> clear`，呼叫 `parse("clear")` |
| t1 | `parse` | 切成 `["clear"]`，依序和 `add`、`remove`、`list`、`quit` 四條 pattern 比對，全部不符 |
| t2 | `parse` | 掉進 `_`，印「無法解析」，回傳 `None` |
| t3 | `main` | `None => true`，`execute` 沒有被呼叫 |
| 永遠不會發生 | `execute` | `Command::Clear => bag.clear()` |

`Command::Clear` 這個值只有 `parse` 能造出來。`parse` 不造，`execute` 那條 arm 寫得再對也到不了。

這條線上撞到的三件事：

| 當時的想法 | 修正 |
|---|---|
| 回傳型別是 `Option<Command>`，所以要滿足所有成員 | 回傳是「交出其中一個」，交哪個是自由的。必須涵蓋所有成員的只有 `match`：收的人要全部準備好，給的人只要給一個 |
| 寫了 `fn clear() -> Command { Command::Clear }` 就算做出 `Clear` 了 | 寫 fn 是寫下做法，要有人呼叫才會跑。rustc 從 `main` 出發只追走得到的程式碼，沒人呼叫的 fn 裡面那行等於不存在，所以同時出現 `associated function clear is never used` |
| `Clear` 需要「實作」 | `Clear` 是一個值，像數字 `5`，只有「有沒有人寫出來用」。要實作的是「收到 `Clear` 做什麼」，那在 `execute` |

同一個「忘記處理新指令」，兩個方向的保護強度不同：

| 方向 | 函式 | 忘記處理 `Clear` | 為什麼 |
|---|---|---|---|
| `Command` → 行為 | `execute` | **error**（E0004），編不過 | 輸入是 enum，可能的值有限，rustc 能逐一清點 |
| `&str` → `Command` | `parse` | 只有 **warning** | 輸入是字串，可能的值無限，`_ =>` 本來就必須存在，rustc 無從知道漏了哪個字 |

### 實驗 B

印出下面六個型別的 `std::mem::size_of`，先寫下你預測的數字：
`u32`、`Option<u32>`、`&str`、`Option<&str>`、`Command`、`Option<Command>`

**預測**：

```
u32 -> 4
Option<u32> -> 8 (因為u32吃滿空間所以額外空間對齊後8)
&str -> 16
Option<&str> -> 16 (我有偷查，位址為空就是None的意思)
Command -> 24+4=28 -> 32 (Max是Add(String,u32))
Option<Command> -> 32 (enum的Key可作None)
```

**結果**：

**複盤**：六個數字全對。實測：

```
u32             4
Option<u32>     8
&str            16
Option<&str>    16
Command         32
Option<Command> 32
```

`Command` 的推理少一步：「24 + 4 = 28 → 32」沒有算 tag。enum 要記住「現在是哪個 variant」，照理要多一格 tag。這裡沒有變大，是因為 rustc 把 tag 藏進 `String` 的不可能值裡：`String` 的 `cap` 永遠 ≤ `isize::MAX`，最高位元是 1 的值全部用不到。

實測各個值的 32 bytes，每 8 bytes 一格（rustc 1.97、x86-64）：

```
                  第 0 格              第 1 格    第 2 格    第 3 格
Add("sword", 2)   cap = 5              ptr        len = 5    2
Remove("sword")   0x8000000000000000   cap = 5    ptr        len = 5
List              0x8000000000000001   （未使用）
Quit              0x8000000000000002   （未使用）
None::<Command>   0xFFFFFFFFFFFFFFFF   （未使用）
```

第 0 格是合法的 `cap` 就是 `Add`，否則那個不可能值本身就是 tag。`Option<Command>` 的 `None` 再拿一個還沒用掉的不可能值，所以還是 32。你寫的「enum 的 Key 可作 None」方向正確。

這個版面是目前 rustc 的選擇，不是語言保證，換版本可能不同。有保證的只有 `Option<&T>` 這一類（見問題 4）。

## 問題

回答直接寫在各題下面。

### 問題 1

實驗 B：`Option<u32>` 比 `u32` 大，`Option<&str>` 卻和 `&str` 一樣大。`None` 在這兩種情況下各自是怎麼被記錄的？

**回答**：

```
Option<32> 因為 u32 沒空間可借表示None所以對齊後變大。
Option<&str> 使用指標表示None，所以不變。
```

**複盤**：正確。補上精確的說法：關鍵是這個型別有沒有「不可能值」（niche）。

| 型別 | 所有 bit pattern 都是合法值嗎 | `None` 記在哪 | 大小 |
|---|---|---|---|
| `u32` | 是，2³² 種全部用掉 | 另外加一格 tag（1 byte），再補齊到對齊 4 的倍數 | 4 → 8 |
| `&str` | 否，reference 的 ptr 不可能是 0 | ptr = 0 就代表 `None`，不必加格子 | 16 → 16 |

修一個詞：不是「用指標表示 None」，是「用指標**不可能出現的值 0** 表示 None」。

### 問題 2

如果 `execute` 用了 `_ => {}`，實驗 A 會發生什麼？哪一種寫法會在「忘記處理新指令」時保護你？這個保護是在編譯期還是執行期生效？

**回答**：

```
使用default => {}，這在compile可以pass，但是runtime執行會錯誤。
實驗A則是 compile會反饋錯誤，因為沒有match所有struct。
```

**複盤**：後半對：四個 variant 全列的寫法，漏掉 `Clear` 會在編譯期被擋下。

前半的模型要修：「`_ =>` compile 可以 pass，但 runtime 執行會錯誤」。執行期**不會有任何錯誤**。`Clear` 安靜地掉進 `_` 那條 arm，執行那條 arm 的內容，沒有 panic、沒有訊息。

實測：把 `execute` 最後一條換成 `_ => true`，背包裡有 sword 時輸入 `clear` 再 `list`：

```
> clear
> list
sword x3        ← 沒被清空，程式也沒有任何抱怨
> quit
```

| 寫法 | 新增 `Clear` 卻忘了處理 | 誰在什麼時候發現 |
|---|---|---|
| variant 全列 | E0004，編不過 | rustc，編譯期 |
| `_ => …` | 編得過、跑得動、行為是錯的 | 沒有人通知你，等使用者回報 |

`_` 的意思是「其他情況我都不在乎」，rustc 照字面相信你。這比 runtime error 更糟：error 至少會讓你知道出事了。

修一個詞：「沒有 match 所有 struct」應該是 variant。`Command` 是**一個**型別，`Add` / `Remove` / `List` / `Quit` 是這個型別的值的四種形狀，不是四個 struct。

### 問題 3

`execute` 裡 `match cmd { Command::Add(name, qty) => … }` 之後，`name` 的擁有者是誰？把它 `push` 進 `bag` 時，heap 上的字元有被複製嗎？

**回答**：

```
name型別是String，
Command::Add(name,qty) => ... 
這段執行時，name是被move了，所以execute完畢後就被釋放。
push進bag時候，又move了一次，heap狀態都沒有變，變的是stack上持有的是哪個frame中對應變數位址。(我猜想，這邊應該就是frame的概念，但是否存在不會有frame的情境，應該不會？畢竟這Add就是透過fn invoke?幫我解答。)
```

**複盤**：題目用的名字是 `name` / `qty`，你的程式裡叫 `item` / `count`，下面沿用題目的。

對的部分：`match cmd` 把 `String` 從 `cmd` 搬出來；`push` 時 heap 上的字元沒有被複製。實測（位址每次執行都不同，看的是相不相同、落在哪一區）：

```
push 前  name 的字元位址      0x559e80c74da0    name 本身（ptr/len/cap）在  0x7fff4e5114e0   ← stack
push 後  bag[0].0 的字元位址  0x559e80c74da0    bag[0].0 本身在             0x559e80c74dc0   ← heap
```

要修三處。

**1. 擁有者沒有答到。** `name` 的擁有者是 `name` 這個 binding，它是 `execute` 的 frame 裡的一格。搬移之前的擁有者是參數 `cmd`，也在 `execute` 的 frame。

**2.（模型）「被 move 了，所以 execute 完畢後就被釋放」不一定成立**，要看走哪條路。這和 q03 的 anvil 是同一件事：

| 輸入 | 走的 arm | `name` 的下場 |
|---|---|---|
| `add sword 2`（背包沒有） | `None => bag.push((name, qty))` | 再搬進 `bag`，**不釋放**，活到 `bag` 消失 |
| `add sword 1`（背包已有） | `Some(..) => *quantity += qty` | 沒人接手，在 `Add` 這條 arm 的 `}` **釋放**，比 `execute` 返回還早 |

實測（把 `String` 包一層並實作 `Drop` 來觀察）：

```
add sword 2
    (Add 的 arm 最後一行)
    (match 結束、execute 還沒 return)
add sword 1
    (Add 的 arm 最後一行)
    drop: sword                          ← arm 結束就釋放
    (match 結束、execute 還沒 return)
main 結束
    drop: sword                          ← 第一把，跟著 bag 一起
```

> 釋放發生在**最後一個擁有者**消失的地方。move 只是換擁有者，不會安排釋放。

**3.（模型，之前出現過）「變的是 stack 上哪個 frame 的變數持有」。** `push` 之後，那 24 bytes（ptr / len / cap）不在任何 frame 裡，它被搬進 `bag` 的 heap 緩衝區，就是上面實測第二行右邊的位址。擁有者可以住在 heap：

```
STACK                                    HEAP
main 的 frame
  bag: [ptr | len 1 | cap 4] ────────►  [ (String{ptr,len,cap}, 2) | 空 | 空 | 空 ]
                                                    │
                                                    └────────────►  "sword"
```

擁有關係是一棵樹：`bag` → 緩衝區裡的 tuple → `String` → 字元。只有樹根在 stack。

所以「heap 狀態都沒有變」也要修：字元那一塊沒變，但 `bag` 的緩衝區（也在 heap）被寫入了 32 bytes，滿了還會重新配置。

**你問的：「是否存在不會有 frame 的情境？畢竟這 Add 就是透過 fn invoke？」**

這裡的 `Command::Add(name, qty)` 不是函式呼叫。同一種寫法出現在兩種位置，意思相反：

| 位置 | 例子 | 意思 | 有沒有 call、新 frame |
|---|---|---|---|
| expression（`=>` 右邊、一般敘述） | `parse` 裡的 `Command::Add(String::from(*item), n)` | 組裝：把欄位和 tag 寫進一塊記憶體 | 沒有。rustc 直接產生寫入欄位的指令 |
| pattern（`=>` 左邊、`let` 左邊） | `execute` 裡的 `Command::Add(name, qty) =>` | 拆解：檢查 tag，替欄位取名字 | 沒有。是 `execute` 自己 frame 裡的比較和搬移 |

「有沒有不存在 frame 的情境」分兩層答：

- **正在執行的程式碼**：一定屬於某個函式。但 frame 是機器層的東西，函式被 inline 之後不會有自己的 frame，區域變數併進呼叫者的 frame，甚至只活在暫存器裡。「一次呼叫 = 一個 frame」是未最佳化時的模型。
- **值**：不一定住在 frame。`static` 在 `.data` / `.bss`，字串字面值在 `.rodata`，`Vec` 的元素在 heap。

### 問題 4

Rust 沒有 `null`。那 `Option<&str>` 的 `None` 在機器層實際上長什麼樣子？它和 C 的 `NULL` 指標，差別在哪一層？

**回答**：

```
我猜測，
rust是透過空指標表示None，是什麼樣子應該8bytes都是0(幫我解答)
C的也是8byptes都是0。
我有偷查，C可以操作空指標，rust則是必須斷言非空才可以，是這差異嗎？(幫我解答)
```

**複盤**：對，`None` 就是 ptr 為 0，和 C 的 `NULL` 在機器層是同一個 bit pattern。實測 `Option<&u32>`：

```
Some(&x) = 0x00007fff4e51119c
None     = 0x0000000000000000
```

修一個數字：`Option<&str>` 是 16 bytes（ptr + len）。`None` 時 ptr 那 8 bytes 是 0，len 那 8 bytes 沒有意義，不會被讀。

你查到的差異是對的，修一個詞：不是「斷言非空」，是「**拆開**才拿得到」。差別在編譯器（型別系統）這一層，不在機器層：

| 層 | C：`char *p` | Rust：`Option<&str>` |
|---|---|---|
| 記憶體 | 8 bytes 全 0 | ptr 的 8 bytes 全 0 |
| 機器碼 | 你有寫 `if (p != NULL)` 才有一道比較 | `match` / `if let` 編出同樣一道「和 0 比較」 |
| 型別系統 | `char *` 這個型別**包含** NULL，沒檢查就用，照樣編過 | `&str` 這個型別**不包含** null。`Option<&str>` 是另一個型別，上面沒有 `.len()` 這些方法，不拆開就編不過 |
| 忘記檢查的下場 | 執行期：對位址 0 解參考是未定義行為，通常是 MMU 查不到對應 → page fault → kernel 送 SIGSEGV | 編譯期：error，執行檔產生不出來 |

這和問題 1 是同一件事的兩面：正因為 `&str` 保證不是 0，0 才空得出來給 `None` 用。

「斷言」對應的是 `unwrap()`：它也是拆開，只是 `None` 那條路寫成 panic，執行期才爆。所以這題的 `parse` 禁用。

## 程式碼複盤

第四輪，通過。輸出和預期逐行相同，沒有 warning。多出來的 `> clear`、`> list` 兩行是你加進 `lines` 驗證 `Clear` 用的，`clear` 之後的 `list` 沒有印任何東西，代表背包確實清空了。

✅ 第四輪解決的

1. `parse` 不再自己印「無法解析」：`_` 改成 `_ => None`，由 `main` 決定怎麼處理：

   ```rust
   let Some(cmd) = Command::parse(line) else {
       println!("無法解析");
       continue;
   };
   ```

   `let … else`：拆得開就把裡面的值綁到 `cmd`；拆不開就走 `else`，而 `else` 裡必須離開（`continue` / `break` / `return`）。

   **為什麼 `parse` 不該印**（第二輪問的）：rustc 不會擋，這題的輸出也剛好正確。問題是 `println!` 把「解析失敗要怎麼處理」寫死了，而這件事只有呼叫的人才知道。

   簽名 `&str -> Option<Command>` 是 `parse` 對外的全部承諾：給字串，回報有或沒有。印字是簽名上看不到的額外動作，每個呼叫者都被迫接受。

   實測，加第二個呼叫者，只想數有幾行是合法指令：

   ```rust
   let valid = lines.iter().filter(|l| Command::parse(l).is_some()).count();
   println!("合法指令 {valid} 行");
   ```

   ```
   無法解析
   無法解析
   無法解析
   合法指令 9 行
   ```

   三行沒有人要的輸出，呼叫者關不掉。

   | 呼叫者 | 解析失敗時想做的事 | `parse` 自己印的後果 |
   |---|---|---|
   | 這題的 `main` | 印「無法解析」 | 剛好一樣 |
   | 只想驗證或計數 | 什麼都不印 | 多出雜訊 |
   | 單元測試（q07） | 檢查回傳值是 `None` | 測試只驗得到回傳值，印出來的字驗不到 |
   | server（q12） | 把錯誤訊息寫回**那條連線的 socket** | 字印在 server 行程自己的 stdout，打錯指令的人什麼都沒收到 |

   > 偵測的人回報，呼叫的人決定。`parse` 只負責把「失敗」放進回傳值；要印、要忽略、要送去哪，是呼叫者的事。

✅ 前三輪解決的

- 輸出：`Add`、`Remove` 多印的行拿掉了，`No item` 改成 `找不到:`。
- `Clear`：variant、`execute`、`parse` 三處都接上，`lines` 也加了 `clear` / `list` 來驗證。

🟡 沒改的建議（不影響結案）

- 註解掉的程式碼（`main` 裡舊的那段 `match`、`execute` 裡兩行 `println!`）直接刪掉。不要的程式碼不留在檔案裡，git 會記得。
- `fn clear() -> Command { Command::Clear }` 沒有帶來任何東西，刪掉，`parse` 直接寫 `["clear"] => Some(Command::Clear)`。
- `let rs = match cmd { … }; rs`：`match` 本身就是 expression，直接當函式的最後一個 expression，`rs` 可以拿掉。
- `execute(cmd: Command, …)` 寫在 `impl Command` 裡卻沒有 `self`，所以呼叫要寫成 `Command::execute(cmd, &mut bag)`。第一個參數改成 `self` 就能寫 `cmd.execute(&mut bag)`。這就是 q03 的 `drop_all(self)`：把自己吃掉。
- `if let` guard（`[…] if let Ok(n) = …`）是近期才穩定的語法，舊一點的 toolchain 編不過。不靠 guard 的寫法：

  ```rust
  ["add", item, count] => count
      .parse()
      .ok()
      .map(|n| Command::Add(item.to_string(), n)),
  ```

  `.ok()` 把 `Result` 變成 `Option`，`.map` 只在 `Some` 時組出 `Add`，數量不是數字就原樣回 `None`。
- `collect::<Vec<&str>>()` 每一行都會配置一塊 heap（`Vec` 的緩衝區，裡面是指向 `line` 的 `&str`，字元沒有被複製）。這題的規模無所謂；題目提示的 `split_whitespace()` + `.next()` 是不配置的寫法。
- 兩個實驗的「結果」沒有填，實測結果都寫在各自的複盤裡。

🟢 做得好

- slice pattern（`["add", item, count]`）：長度和內容一次比對完。`list foo`、`add a 1 extra` 這種多帶參數的輸入也會被擋掉，比逐個 `.next()` 嚴謹。
- 兩條限制都守住：`execute` 四個 variant 全列、沒有 `_`；`parse` 沒有 `unwrap` / `expect`。
- `Add`：`iter_mut().find(…)` 拿到 `&mut` 直接改數量，只有 `None` 那條路才把 `item` 搬進背包，沒有多餘的 `clone`。
- `Remove`：先用 `position` 拿到 index，借用結束之後才 `remove`。先查再改，避開了「邊走訪邊刪除」。
- `match cmd` 用值來 match，`String` 從指令直接搬進背包，全程沒有複製字元。
- 實驗 B 六個數字全對。`Command` 取最大的 variant 這一步，是之前卡過的地方。

## 交卷

commit 之後說「複盤 q04」。
