# q07：mini grep

檔案：這題第一次把一個 binary 拆成多個檔案，改用資料夾形式

```
src/bin/q07_minigrep/
├── main.rs      # 進入點：讀參數、呼叫、決定結束代碼
├── config.rs    # 解析命令列參數
└── search.rs    # 搜尋邏輯與測試
```

測試資料：[`data/sample.txt`](../data/sample.txt)

## 背景

做一個簡化版的 `grep`：在檔案裡找出包含某個字串的每一行。

```bash
cargo run --bin q07_minigrep -- [-i] [-n] <pattern> <file>
```

`--` 後面的東西是給你的程式的，cargo 不會解讀。

## 要做的事

1. `config.rs`：
   ```rust
   pub struct Config {
       pub pattern: String,
       pub path: String,
       pub ignore_case: bool,   // -i
       pub line_numbers: bool,  // -n
   }
   ```
   `pub fn parse(args: &[String]) -> Result<Config, String>`，參數不對時 `Err` 裡放用法說明。

2. `search.rs`：
   ```rust
   pub fn search<'a>(pattern: &str, contents: &'a str, ignore_case: bool) -> Vec<(usize, &'a str)>
   ```
   回傳每個符合的行：行號（從 1 開始）和那一行。

3. `main.rs`：
   - `fn run(config: &Config) -> Result<bool, Box<dyn std::error::Error>>`：讀檔、搜尋、印出結果，回傳「有沒有找到」。讀檔用 `std::fs::read_to_string` 配 `?`
   - 符合的行印到 stdout；有 `-n` 時前面加 `<行號>:`
   - 錯誤訊息用 `eprintln!` 印到 stderr
   - 結束代碼比照 `grep`：找到 `0`、沒找到 `1`、發生錯誤 `2`（用 `std::process::exit`）

4. 在 `search.rs` 寫至少三個單元測試（`#[cfg(test)] mod tests`），用 `cargo test --bin q07_minigrep` 執行。

5. 對照預期結果：

   | 指令（省略 `cargo run --bin q07_minigrep --`） | 預期 |
   |---|---|
   | `The data/sample.txt` | 第 1、2、5 行 |
   | `-i The data/sample.txt` | 第 1、2、3、5 行 |
   | `-n value data/sample.txt` | `3:…` 和 `4:…` 兩行 |
   | `zzz data/sample.txt` | 沒有輸出，結束代碼 `1` |
   | `The nope.txt` | stderr 有錯誤訊息，結束代碼 `2` |
   | `The` | stderr 有用法說明，結束代碼 `2` |

## 限制

- 不准出現 `unwrap` / `expect`（測試裡可以）
- `search` 不准複製任何一行的內容，回傳的 `&str` 必須指向 `contents` 裡面

## 實驗

每個都先預測，再試。預測和結果直接寫在各實驗下面。

### 實驗 A

把 `search` 前面的 `pub` 拿掉

**預測**：

**結果**：

### 實驗 B

分別執行這兩行，觀察畫面上還看得到什麼、`out.txt` 裡有什麼：

```bash
cargo run -q --bin q07_minigrep -- The data/sample.txt > out.txt
cargo run -q --bin q07_minigrep -- The nope.txt > out.txt
```

**預測**：

**結果**：

### 實驗 C

把 `search` 簽名裡的三個 `'a` 全部拿掉

**預測**：

**結果**：

## 問題

回答直接寫在各題下面。

### 問題 1

`main.rs` 裡的 `mod search;` 這一行讓 rustc 去哪裡找檔案？拆成三個檔案之後，cargo 為了這個 binary 呼叫了幾次 rustc？

**回答**：

### 問題 2

實驗 B：`>` 只導走了哪一個？這個「導向」是誰做的、在你的程式開始執行之前還是之後？

**回答**：

### 問題 3

實驗 C：編譯器為什麼不能自己推斷？`'a` 在這裡告訴了編譯器什麼事？

**回答**：

### 問題 4

`search` 回傳的那些 `&str` 指向哪一塊記憶體？那塊記憶體的擁有者是哪個變數、在哪個函式的 frame 裡、活到什麼時候？

**回答**：

### 問題 5

`read_to_string` 從你的程式一路到磁碟，中間經過哪幾層？每一層各做什麼？

**回答**：

## 交卷

commit 之後說「複盤 q07」。
