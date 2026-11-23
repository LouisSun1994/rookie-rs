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

## 問題

回答直接寫在各題下面。

### 問題 1

實驗 B：`Option<u32>` 比 `u32` 大，`Option<&str>` 卻和 `&str` 一樣大。`None` 在這兩種情況下各自是怎麼被記錄的？

**回答**：

```
Option<32> 因為 u32 沒空間可借表示None所以對齊後變大。
Option<&str> 使用指標表示None，所以不變。
```

### 問題 2

如果 `execute` 用了 `_ => {}`，實驗 A 會發生什麼？哪一種寫法會在「忘記處理新指令」時保護你？這個保護是在編譯期還是執行期生效？

**回答**：

```
使用default => {}，這在compile可以pass，但是runtime執行會錯誤。
實驗A則是 compile會反饋錯誤，因為沒有match所有struct。
```

### 問題 3

`execute` 裡 `match cmd { Command::Add(name, qty) => … }` 之後，`name` 的擁有者是誰？把它 `push` 進 `bag` 時，heap 上的字元有被複製嗎？

**回答**：

```
name型別是String，
Command::Add(name,qty) => ... 
這段執行時，name是被move了，所以execute完畢後就被釋放。
push進bag時候，又move了一次，heap狀態都沒有變，變的是stack上持有的是哪個frame中對應變數位址。(我猜想，這邊應該就是frame的概念，但是否存在不會有frame的情境，應該不會？畢竟這Add就是透過fn invoke?幫我解答。)
```

### 問題 4

Rust 沒有 `null`。那 `Option<&str>` 的 `None` 在機器層實際上長什麼樣子？它和 C 的 `NULL` 指標，差別在哪一層？

**回答**：

```
我猜測，
rust是透過空指標表示None，是什麼樣子應該8bytes都是0(幫我解答)
C的也是8byptes都是0。
我有偷查，C可以操作空指標，rust則是必須斷言非空才可以，是這差異嗎？(幫我解答)
```

## 交卷

commit 之後說「複盤 q04」。
