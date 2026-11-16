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

每個都先預測，再試。

- A. 在 `Command` 加一個 variant `Clear`，先不要改 `execute`，直接編譯。看完結果再把 `Clear`（清空背包）做完
- B. 印出下面六個型別的 `std::mem::size_of`，先寫下你預測的數字：
  `u32`、`Option<u32>`、`&str`、`Option<&str>`、`Command`、`Option<Command>`

## 在註解裡回答（先寫預測，再跑）

1. 實驗 B：`Option<u32>` 比 `u32` 大，`Option<&str>` 卻和 `&str` 一樣大。`None` 在這兩種情況下各自是怎麼被記錄的？
2. 如果 `execute` 用了 `_ => {}`，實驗 A 會發生什麼？哪一種寫法會在「忘記處理新指令」時保護你？這個保護是在編譯期還是執行期生效？
3. `execute` 裡 `match cmd { Command::Add(name, qty) => … }` 之後，`name` 的擁有者是誰？把它 `push` 進 `bag` 時，heap 上的字元有被複製嗎？
4. Rust 沒有 `null`。那 `Option<&str>` 的 `None` 在機器層實際上長什麼樣子？它和 C 的 `NULL` 指標，差別在哪一層？

## 交卷

commit 之後說「複盤 q04」。
