# q01：Collatz 步數

檔案：[`src/bin/q01_collatz.rs`](../src/bin/q01_collatz.rs)

## 規則

對一個正整數 n 重複做下面的事，直到 n 變成 1：

- n 是偶數 → `n / 2`
- n 是奇數 → `3n + 1`

例：`6 → 3 → 10 → 5 → 16 → 8 → 4 → 2 → 1`，共 8 步。

## 要做的事

1. 寫 `fn collatz_len(n: u64) -> u32`，回傳從 n 走到 1 的步數。
   - `collatz_len(1) == 0`
   - `collatz_len(6) == 8`
2. `main` 印出 1 到 10 每個數的步數。
3. `main` 找出 1 到 999,999 之間步數最多的起點，印出起點和步數。
   - 自我驗證用的正解：起點 `837799`，`524` 步
4. 實驗：把 `collatz_len` 的參數型別改成 `u32`，分別跑 debug 和 `--release` 兩種 build，
   把兩次觀察到的現象寫在檔案最上面的註解。
   - release 版如果一直跑不完就 Ctrl+C，「跑不完」本身也是一個觀察結果
   - 做完實驗把型別改回 `u64`

## 限制

- 只用 `loop` / `while` / `for i in a..b` 和 `if`，先不要用 iterator 方法（`.map`、`.max_by_key` 等）
- 至少有一處把 `if ... else ...` 當成 expression 用：`let x = if ... { ... } else { ... };`
- 不要寫 `return`，讓函式最後一個 expression 當回傳值

## 在註解裡回答（先寫預測，再跑）

1. 第 4 步兩種 build 的行為為什麼不同？那個「檢查」是誰放進去的：CPU、kernel、std，還是 rustc？在哪個時間點放的？
2. 把第 3 步記錄最佳值的那個變數（例如 `let mut best = 0;`）的 `mut` 拿掉會怎樣？錯誤是在編譯期還是執行期出現？

## 交卷

commit 之後說「複盤 q01」。
